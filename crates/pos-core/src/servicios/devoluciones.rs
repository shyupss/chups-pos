//! Devoluciones parciales: el cliente trae de vuelta parte de lo que compró.
//!
//! - No se devuelve más de lo vendido menos lo ya devuelto; por unidad, unidades enteras.
//! - Monto de cada línea = subtotal × cantidad devuelta / cantidad vendida, redondeado al peso.
//!   Se calcula sobre lo acumulado (lo devuelto hasta ahora más esta vez, menos lo que ya se
//!   devolvió), así varias devoluciones de la misma línea suman exactamente su subtotal.
//! - Si la venta tuvo descuento, se descuenta la parte proporcional (también acumulada).
//! - Reembolso en efectivo: sale de la caja abierta y se redondea a la decena como al cobrar.
//!   Con tarjeta o transferencia se registra el medio. En una fiada no se devuelve dinero: baja
//!   la deuda.
//! - "Vuelve al stock": movimiento 'devolucion' que repone el stock. Si no (dañado o vencido),
//!   un movimiento 'merma' que deja el stock igual y anota lo que se perdió.
//! - Cuando todo lo vendido quedó devuelto, la venta pasa a "devuelta".

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::boleta::formato::{cantidad, folio};
use crate::error::validacion;
use crate::repositorios::devoluciones::{self as repo, NuevaDevolucion};
use crate::repositorios::productos::Unidad;
use crate::repositorios::{productos, turnos, ventas};
use crate::servicios::anulacion::{medio_valido, motivo_valido, sin_caja};
use crate::servicios::cantidades::MIL;
use crate::servicios::config;
use crate::servicios::historial::{self, DetalleVenta, EstadoVenta};
use crate::servicios::seguridad::{self, Accion, ControlPin};
use crate::servicios::ventas::{redondeo_a_decena, MedioPago};
use crate::{Error, Resultado};

/// Una línea a devolver. La cantidad va en milésimas, como al vender.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDevuelto {
    pub venta_item_id: i64,
    pub cantidad_mil: i64,
    #[serde(default = "si")]
    pub vuelve_a_stock: bool,
}

fn si() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DevolucionRegistrada {
    pub devolucion_id: i64,
    /// Lo devuelto al cliente (o lo que bajó la deuda), con el redondeo.
    pub monto: i64,
    pub redondeo: i64,
    pub venta: DetalleVenta,
}

/// `parte × num / den` redondeado al entero más cercano (la mitad sube), sin desbordes.
fn proporcion(parte: i64, num: i64, den: i64) -> i64 {
    if den == 0 {
        return 0;
    }
    let (p, n, d) = (i128::from(parte), i128::from(num), i128::from(den));
    ((p * n * 2 + d) / (d * 2)) as i64
}

/// Lo que vale devolver `cantidad` de una línea de la que ya se devolvió `antes`.
pub fn monto_linea(subtotal: i64, vendida: i64, antes: i64, cantidad: i64) -> i64 {
    proporcion(subtotal, antes + cantidad, vendida) - proporcion(subtotal, antes, vendida)
}

/// Registra la devolución. Pide el PIN si "devolución" está protegida.
pub async fn registrar(
    pool: &SqlitePool,
    control: &ControlPin,
    venta_id: i64,
    items: &[ItemDevuelto],
    medio_reembolso: Option<MedioPago>,
    motivo: &str,
    pin: Option<&str>,
) -> Resultado<DevolucionRegistrada> {
    let motivo = motivo_valido(motivo, "devolución")?;
    if items.is_empty() {
        return validacion("Elige al menos un producto para devolver");
    }
    let mut vistos = HashSet::new();
    for i in items {
        if !vistos.insert(i.venta_item_id) {
            return validacion("Cada producto va una sola vez en la devolución");
        }
        if i.cantidad_mil <= 0 {
            return validacion("La cantidad a devolver debe ser mayor que cero");
        }
    }

    let v = historial::detalle(pool, venta_id).await?;
    match v.estado {
        EstadoVenta::Anulada => {
            return validacion("La venta está anulada: no hay nada que devolver")
        }
        EstadoVenta::Devuelta => return validacion("Ya se devolvió todo lo vendido"),
        _ => {}
    }
    let fiada = v.medio_pago == MedioPago::Fiado;
    if fiada && v.tiene_abonos {
        return validacion("La venta fiada tiene abonos: todavía no se puede devolver");
    }
    let medio = medio_valido(if fiada { "fiada" } else { "pagada" }, medio_reembolso)?;
    let efectivo = medio == Some(MedioPago::Efectivo);
    validar_cantidades(&v, items)?;
    // Antes del PIN, para no pedirlo en vano.
    if efectivo
        && crate::servicios::turnos::estado_caja(pool)
            .await?
            .turno
            .is_none()
    {
        return Err(sin_caja());
    }
    seguridad::exigir(pool, control, Accion::Devolucion, pin).await?;

    let mut tx = pool.begin().await?;
    let turno_id = turnos::id_abierto(&mut tx).await?;
    if efectivo && turno_id.is_none() {
        return Err(sin_caja());
    }
    // Las cantidades se vuelven a leer dentro de la transacción: otra devolución pudo llegar antes.
    let lineas = ventas::items_con_devuelto(&mut tx, venta_id).await?;
    let mut por_linea = Vec::with_capacity(items.len());
    let mut suma_lineas = 0;
    for i in items {
        let l = lineas
            .iter()
            .find(|l| l.id == i.venta_item_id)
            .ok_or_else(|| Error::Validacion("Un producto no es de esta venta".into()))?;
        if i.cantidad_mil > l.cantidad_mil - l.devuelto_mil {
            return Err(Error::Conflicto(
                "La venta cambió mientras se devolvía: vuelve a abrirla".into(),
            ));
        }
        let monto = monto_linea(l.subtotal, l.cantidad_mil, l.devuelto_mil, i.cantidad_mil);
        suma_lineas += monto;
        por_linea.push((i, l, monto));
    }

    // Descuento de la venta: la parte que corresponde, sobre lo acumulado.
    let neto = if v.descuento > 0 && v.subtotal > 0 {
        let pagado = v.subtotal - v.descuento;
        let antes: i64 = lineas.iter().map(|l| l.devuelto_monto).sum();
        proporcion(antes + suma_lineas, pagado, v.subtotal) - proporcion(antes, pagado, v.subtotal)
    } else {
        suma_lineas
    };
    let redondeo = if efectivo && config::redondeo_efectivo_activo(&mut tx).await? {
        redondeo_a_decena(neto)
    } else {
        0
    };
    let monto = neto + redondeo;

    let devolucion_id = repo::insertar(
        &mut tx,
        &NuevaDevolucion {
            venta_id,
            turno_id,
            monto,
            redondeo,
            medio_reembolso: medio.map(MedioPago::como_texto),
            motivo: &motivo,
        },
    )
    .await?;
    let referencia = format!("Devolución de la venta N° {}", folio(v.folio));
    for (i, l, monto) in &por_linea {
        repo::insertar_item(
            &mut tx,
            devolucion_id,
            l.id,
            i.cantidad_mil,
            *monto,
            i.vuelve_a_stock,
        )
        .await?;
        if i.vuelve_a_stock {
            productos::mover_stock(
                &mut tx,
                l.producto_id,
                i.cantidad_mil,
                "devolucion",
                Some(venta_id),
                Some(&referencia),
            )
            .await?;
        } else {
            // El producto vuelve dañado o vencido: no entra al stock, pero queda la merma anotada.
            let nota = format!(
                "{referencia}: {} no vuelve al stock (dañado o vencido)",
                cantidad(i.cantidad_mil, l.unidad)
            );
            productos::mover_stock(
                &mut tx,
                l.producto_id,
                0,
                "merma",
                Some(venta_id),
                Some(&nota),
            )
            .await?;
        }
    }
    let todo = lineas.iter().all(|l| {
        let ahora = items
            .iter()
            .find(|i| i.venta_item_id == l.id)
            .map_or(0, |i| i.cantidad_mil);
        l.devuelto_mil + ahora >= l.cantidad_mil
    });
    if !ventas::sumar_devolucion(&mut tx, venta_id, monto, todo).await? {
        return Err(Error::Conflicto(
            "La venta cambió mientras se devolvía: vuelve a abrirla".into(),
        ));
    }
    tx.commit().await?;

    Ok(DevolucionRegistrada {
        devolucion_id,
        monto,
        redondeo,
        venta: historial::detalle(pool, venta_id).await?,
    })
}

/// Cada línea debe ser de la venta, por unidad en unidades enteras y sin pasar de lo que queda.
fn validar_cantidades(v: &DetalleVenta, items: &[ItemDevuelto]) -> Resultado<()> {
    for i in items {
        let Some(l) = v.lineas.iter().find(|l| l.id == i.venta_item_id) else {
            return validacion("Un producto no es de esta venta");
        };
        if l.unidad == Unidad::Unidad && i.cantidad_mil % MIL != 0 {
            return validacion(format!(
                "{} se vende por unidad: devuelve unidades enteras",
                l.nombre
            ));
        }
        let queda = l.cantidad_mil - l.devuelto_mil;
        if i.cantidad_mil > queda {
            return validacion(if queda == 0 {
                format!("{} ya se devolvió completo", l.nombre)
            } else {
                format!(
                    "De {} solo quedan {} por devolver",
                    l.nombre,
                    cantidad(queda, l.unidad)
                )
            });
        }
    }
    Ok(())
}

/// La venta de una devolución (para imprimir su nota).
pub async fn venta_de(pool: &SqlitePool, devolucion_id: i64) -> Resultado<i64> {
    let mut conn = pool.acquire().await?;
    repo::venta_de(&mut conn, devolucion_id)
        .await?
        .ok_or_else(|| Error::NoEncontrado("La devolución no existe".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_monto_es_proporcional_y_suma_exacto() {
        // 3 unidades por $1.000: de a una se devuelven 333, 334 y 333.
        assert_eq!(monto_linea(1000, 3000, 0, 1000), 333);
        assert_eq!(monto_linea(1000, 3000, 1000, 1000), 334);
        assert_eq!(monto_linea(1000, 3000, 2000, 1000), 333);
        // 0,532 kg por $1.224: la mitad son $612.
        assert_eq!(monto_linea(1224, 532, 0, 266), 612);
        assert_eq!(monto_linea(1224, 532, 266, 266), 612);
        // Todo de una vez es el subtotal.
        assert_eq!(monto_linea(1224, 532, 0, 532), 1224);
        assert_eq!(proporcion(i64::MAX / 2, 3, 3), i64::MAX / 2);
    }
}
