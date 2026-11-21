//! Anular una venta completa: repone el stock de todo lo vendido y devuelve el dinero.
//!
//! - Solo ventas pagadas o fiadas sin devoluciones (si ya tiene, se devuelve el resto).
//! - Reembolso en efectivo: sale del efectivo del turno abierto (lo exige) y se redondea a la
//!   decena como al cobrar. Con tarjeta o transferencia se registra el medio y no toca la caja.
//! - Una fiada anulada deja de contar como deuda; si tiene abonos no se anula todavía.
//! - Todo en una transacción: estado de la venta y movimientos de stock tipo 'devolucion'.

use sqlx::SqlitePool;

use crate::boleta::formato::folio;
use crate::boleta::formato::pesos;
use crate::error::validacion;
use crate::repositorios::fiados as fiados_repo;
use crate::repositorios::{productos, turnos, ventas};
use crate::servicios::historial::{self, DetalleVenta};
use crate::servicios::seguridad::{self, Accion, ControlPin};
use crate::servicios::ventas::{redondeo_a_decena, MedioPago};
use crate::servicios::{config, fiados};
use crate::{Error, Resultado};

/// Motivo obligatorio, sin espacios sobrantes y de hasta 200 caracteres.
pub(crate) fn motivo_valido(motivo: &str, que: &str) -> Resultado<String> {
    let motivo = motivo.trim();
    if motivo.is_empty() {
        return validacion(format!("Escribe el motivo de la {que}"));
    }
    if motivo.chars().count() > 200 {
        return validacion("El motivo no puede pasar de 200 caracteres");
    }
    Ok(motivo.to_owned())
}

/// Revisa el medio de reembolso según cómo se pagó. En una venta pagada es obligatorio; en una
/// fiada es opcional: solo se usa si sobra dinero que devolver (lo que ya abonó y no cabe en
/// otras deudas del cliente). Nunca se devuelve "como fiado".
pub(crate) fn medio_valido(fiado: bool, medio: Option<MedioPago>) -> Resultado<Option<MedioPago>> {
    match (fiado, medio) {
        (_, Some(MedioPago::Fiado)) => validacion("El dinero no se puede devolver como fiado"),
        (true, m) => Ok(m),
        (false, None) => validacion("Elige cómo se devuelve el dinero"),
        (false, Some(m)) => Ok(Some(m)),
    }
}

/// Reembolso de `monto` en `medio`: en efectivo se redondea a la decena (si está activo) y
/// exige la caja abierta. Devuelve (monto con redondeo, redondeo).
pub(crate) async fn reembolso_en(
    conn: &mut sqlx::SqliteConnection,
    medio: MedioPago,
    monto: i64,
    turno_id: Option<i64>,
) -> Resultado<(i64, i64)> {
    if medio != MedioPago::Efectivo {
        return Ok((monto, 0));
    }
    if turno_id.is_none() {
        return Err(sin_caja());
    }
    let redondeo = if config::redondeo_efectivo_activo(conn).await? {
        redondeo_a_decena(monto)
    } else {
        0
    };
    Ok((monto + redondeo, redondeo))
}

/// Error para lo que necesita la caja abierta al devolver efectivo.
pub(crate) fn sin_caja() -> Error {
    Error::Validacion("Abre la caja para devolver efectivo".into())
}

/// Anula la venta. Pide el PIN si "anular venta" está protegida. Devuelve el detalle actualizado.
pub async fn anular(
    pool: &SqlitePool,
    control: &ControlPin,
    venta_id: i64,
    motivo: &str,
    medio_reembolso: Option<MedioPago>,
    pin: Option<&str>,
) -> Resultado<DetalleVenta> {
    let motivo = motivo_valido(motivo, "anulación")?;
    let v = historial::detalle(pool, venta_id).await?;
    let estado = match v.estado {
        historial::EstadoVenta::Pagada => "pagada",
        historial::EstadoVenta::Fiada => "fiada",
        historial::EstadoVenta::Anulada => return validacion("La venta ya está anulada"),
        _ => return validacion(
            "La venta tiene devoluciones: devuelve el resto de los productos en vez de anularla",
        ),
    };
    let _ = estado;
    let fiado = v.medio_pago == MedioPago::Fiado;
    let medio = medio_valido(fiado, medio_reembolso)?;
    // Antes del PIN, para no pedirlo en vano (en una fiada, recién se sabe dentro si hay reembolso).
    if !fiado
        && medio == Some(MedioPago::Efectivo)
        && crate::servicios::turnos::estado_caja(pool)
            .await?
            .turno
            .is_none()
    {
        return Err(sin_caja());
    }
    seguridad::exigir(pool, control, Accion::AnularVenta, pin).await?;

    let mut tx = pool.begin().await?;
    let turno_id = turnos::id_abierto(&mut tx).await?;
    // Pagada: se devuelve el total (en efectivo, a la decena: una venta con débito de $1.224
    // anulada en efectivo devuelve $1.220). Fiada: deja de ser deuda, y lo que el cliente ya
    // había abonado pasa como saldo a favor a sus otras deudas; lo que sobre se le devuelve.
    let (medio_reembolso, reembolso, redondeo) = if fiado {
        let f = fiados_repo::venta(&mut tx, venta_id)
            .await?
            .ok_or_else(|| Error::NoEncontrado("La venta no existe".into()))?;
        let credito = f.total - f.devuelto - f.saldo;
        let trasladado = match f.cliente_id {
            Some(cliente) => {
                fiados::trasladar_saldo_a_favor(&mut tx, cliente, venta_id, credito, turno_id)
                    .await?
            }
            None => 0,
        };
        let resto = credito - trasladado;
        if resto > 0 {
            let Some(m) = medio else {
                return validacion(format!(
                    "El cliente ya había abonado {} que no caben en otras deudas: elige cómo se le devuelve",
                    pesos(resto)
                ));
            };
            let (monto, redondeo) = reembolso_en(&mut tx, m, resto, turno_id).await?;
            (Some(m), Some(monto), Some(redondeo))
        } else {
            (None, None, None)
        }
    } else {
        let m = medio.expect("validado arriba");
        let (monto, redondeo) = reembolso_en(&mut tx, m, v.total, turno_id).await?;
        (Some(m), Some(monto), Some(redondeo))
    };
    let anulada = ventas::anular(
        &mut tx,
        venta_id,
        &ventas::DatosAnulacion {
            motivo: &motivo,
            turno_id,
            medio_reembolso: medio_reembolso.map(MedioPago::como_texto),
            reembolso,
            redondeo,
        },
    )
    .await?;
    if !anulada {
        return Err(Error::Conflicto(
            "La venta cambió mientras se anulaba: vuelve a abrirla".into(),
        ));
    }
    let nota = format!("Anulación de la venta N° {}", folio(v.folio));
    for l in &v.lineas {
        productos::mover_stock(
            &mut tx,
            l.producto_id,
            l.cantidad_mil,
            "devolucion",
            Some(venta_id),
            Some(&nota),
        )
        .await?;
    }
    tx.commit().await?;
    historial::detalle(pool, venta_id).await
}
