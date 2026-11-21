use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::validacion;
use crate::repositorios::productos::Unidad;
use crate::repositorios::{productos, turnos, ventas};
use crate::servicios::{cantidades, config};
use crate::{Error, Resultado};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MedioPago {
    Efectivo,
    Debito,
    Credito,
    Transferencia,
    Fiado,
}

impl MedioPago {
    pub fn desde_texto(texto: &str) -> Option<Self> {
        Some(match texto {
            "efectivo" => MedioPago::Efectivo,
            "debito" => MedioPago::Debito,
            "credito" => MedioPago::Credito,
            "transferencia" => MedioPago::Transferencia,
            "fiado" => MedioPago::Fiado,
            _ => return None,
        })
    }

    /// Nombre para mostrar e imprimir.
    pub fn nombre(self) -> &'static str {
        match self {
            MedioPago::Efectivo => "Efectivo",
            MedioPago::Debito => "Débito",
            MedioPago::Credito => "Crédito",
            MedioPago::Transferencia => "Transferencia",
            MedioPago::Fiado => "Fiado",
        }
    }

    pub fn como_texto(self) -> &'static str {
        match self {
            MedioPago::Efectivo => "efectivo",
            MedioPago::Debito => "debito",
            MedioPago::Credito => "credito",
            MedioPago::Transferencia => "transferencia",
            MedioPago::Fiado => "fiado",
        }
    }
}

/// Un producto del carrito. La cantidad va en milésimas: 3 unidades = 3000, 0,532 kg = 532.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemPedido {
    pub producto_id: i64,
    pub cantidad_mil: i64,
}

/// Lo que envía la caja al cobrar. Los precios NO vienen de la interfaz:
/// se leen de la base al confirmar, para que el total siempre sea el correcto.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuevaVenta {
    pub items: Vec<ItemPedido>,
    pub medio_pago: MedioPago,
    /// Solo en efectivo: lo que entregó el cliente.
    pub monto_recibido: Option<i64>,
    #[serde(default)]
    pub descuento: i64,
    pub cliente_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LineaVendida {
    pub producto_id: i64,
    pub nombre: String,
    pub unidad: Unidad,
    pub precio_unitario: i64,
    pub cantidad_mil: i64,
    pub subtotal: i64,
    pub stock_resultante_mil: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VentaConfirmada {
    pub id: i64,
    pub folio: i64,
    pub fecha_hora: String,
    pub subtotal: i64,
    pub descuento: i64,
    /// Ajuste del efectivo a la decena (puede ser negativo). total = subtotal - descuento + redondeo.
    pub redondeo: i64,
    pub total: i64,
    pub medio_pago: MedioPago,
    pub monto_recibido: Option<i64>,
    pub vuelto: Option<i64>,
    pub lineas: Vec<LineaVendida>,
}

fn desborde() -> Error {
    Error::Validacion("El monto de la venta es demasiado grande".into())
}

/// Ajuste para llevar un monto a la decena, porque en Chile no hay monedas de 1 ni de 5 pesos.
/// Si termina en 1 a 5 baja y si termina en 6 a 9 sube: 1224 → -4, 1225 → -5, 1226 → +4.
pub fn redondeo_a_decena(monto: i64) -> i64 {
    let resto = monto.rem_euclid(10);
    if resto <= 5 {
        -resto
    } else {
        10 - resto
    }
}

/// Confirma una venta en UNA transacción: cabecera, líneas, descuento de stock y
/// movimientos. Si algo falla (o se corta la luz) no queda nada a medias.
///
/// Necesita la caja abierta: la venta queda en el turno (una consulta por el índice único).
///
/// El stock puede quedar negativo: en un negocio real no se debe bloquear una venta
/// porque el inventario esté desactualizado. Se corrige con un ajuste.
pub async fn confirmar(pool: &SqlitePool, v: &NuevaVenta) -> Resultado<VentaConfirmada> {
    if v.items.is_empty() {
        return validacion("La venta no tiene productos");
    }
    // Junta líneas repetidas del mismo producto, conservando el orden de primera aparición.
    // Cada cantidad se valida contra la unidad del producto más abajo, ya con el producto leído.
    let mut orden: Vec<i64> = Vec::new();
    let mut cantidades_mil: BTreeMap<i64, i64> = BTreeMap::new();
    for item in &v.items {
        if item.cantidad_mil <= 0 {
            return validacion("Cada producto debe tener una cantidad mayor que cero");
        }
        let acumulado = cantidades_mil.entry(item.producto_id).or_insert_with(|| {
            orden.push(item.producto_id);
            0
        });
        *acumulado = acumulado
            .checked_add(item.cantidad_mil)
            .ok_or_else(desborde)?;
    }
    if v.medio_pago == MedioPago::Fiado && v.cliente_id.is_none() {
        return validacion("Para fiar hay que elegir un cliente");
    }
    if v.medio_pago != MedioPago::Fiado && v.cliente_id.is_some() {
        return validacion("Solo las ventas fiadas llevan cliente");
    }

    let mut tx = pool.begin().await?;
    let turno_id = turnos::id_abierto(&mut tx)
        .await?
        .ok_or_else(|| Error::Validacion("Abre la caja para vender".into()))?;

    let mut lineas = Vec::with_capacity(orden.len());
    let mut subtotal: i64 = 0;
    for producto_id in &orden {
        let cantidad_mil = cantidades_mil[producto_id];
        let p = productos::por_id(&mut tx, *producto_id)
            .await?
            .filter(|p| p.activo)
            .ok_or_else(|| {
                Error::NoEncontrado("Un producto de la venta ya no existe o fue desactivado".into())
            })?;
        cantidades::validar_venta(p.unidad, &p.nombre, cantidad_mil)?;
        let sub = cantidades::subtotal(p.precio_venta, cantidad_mil).ok_or_else(desborde)?;
        subtotal = subtotal.checked_add(sub).ok_or_else(desborde)?;
        lineas.push(LineaVendida {
            producto_id: p.id,
            nombre: p.nombre,
            unidad: p.unidad,
            precio_unitario: p.precio_venta,
            cantidad_mil,
            subtotal: sub,
            stock_resultante_mil: 0,
        });
    }

    if v.descuento < 0 || v.descuento > subtotal {
        return validacion("El descuento debe estar entre 0 y el subtotal");
    }
    let a_pagar = subtotal - v.descuento;
    // El redondeo lo decide el servicio, no la interfaz: solo efectivo y solo si está activo.
    let redondeo = if v.medio_pago == MedioPago::Efectivo
        && config::redondeo_efectivo_activo(&mut tx).await?
    {
        redondeo_a_decena(a_pagar)
    } else {
        0
    };
    let total = a_pagar.checked_add(redondeo).ok_or_else(desborde)?;

    let (monto_recibido, vuelto) = match v.medio_pago {
        MedioPago::Efectivo => {
            let recibido = v.monto_recibido.unwrap_or(total);
            if recibido < total {
                return validacion("El monto recibido es menor que el total");
            }
            (Some(recibido), Some(recibido - total))
        }
        MedioPago::Fiado => (None, None),
        _ => (Some(total), Some(0)),
    };
    let estado = if v.medio_pago == MedioPago::Fiado {
        "fiada"
    } else {
        "pagada"
    };

    let cabecera = ventas::insertar_cabecera(
        &mut tx,
        &ventas::NuevaCabecera {
            turno_id,
            subtotal,
            descuento: v.descuento,
            redondeo,
            total,
            medio_pago: v.medio_pago.como_texto(),
            monto_recibido,
            vuelto,
            estado,
            cliente_id: v.cliente_id,
        },
    )
    .await?;

    for l in &mut lineas {
        ventas::insertar_item(
            &mut tx,
            cabecera.id,
            &ventas::NuevoItem {
                producto_id: l.producto_id,
                nombre: &l.nombre,
                unidad: l.unidad,
                precio_unitario: l.precio_unitario,
                cantidad_mil: l.cantidad_mil,
                subtotal: l.subtotal,
            },
        )
        .await?;
        l.stock_resultante_mil = productos::mover_stock(
            &mut tx,
            l.producto_id,
            -l.cantidad_mil,
            "venta",
            Some(cabecera.id),
            None,
        )
        .await?;
    }

    tx.commit().await?;

    Ok(VentaConfirmada {
        id: cabecera.id,
        folio: cabecera.folio,
        fecha_hora: cabecera.fecha_hora,
        subtotal,
        descuento: v.descuento,
        redondeo,
        total,
        medio_pago: v.medio_pago,
        monto_recibido,
        vuelto,
        lineas,
    })
}

#[cfg(test)]
mod tests {
    use super::redondeo_a_decena;

    #[test]
    fn redondea_a_la_decena() {
        assert_eq!(redondeo_a_decena(1220), 0);
        assert_eq!(redondeo_a_decena(1221), -1);
        assert_eq!(redondeo_a_decena(1224), -4);
        assert_eq!(redondeo_a_decena(1225), -5);
        assert_eq!(redondeo_a_decena(1226), 4);
        assert_eq!(redondeo_a_decena(1229), 1);
        assert_eq!(redondeo_a_decena(3), -3);
    }
}
