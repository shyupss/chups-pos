//! Anular una venta completa: repone el stock de todo lo vendido y devuelve el dinero.
//!
//! - Solo ventas pagadas o fiadas sin devoluciones (si ya tiene, se devuelve el resto).
//! - Reembolso en efectivo: sale del efectivo del turno abierto (lo exige) y se redondea a la
//!   decena como al cobrar. Con tarjeta o transferencia se registra el medio y no toca la caja.
//! - Una fiada anulada deja de contar como deuda; si tiene abonos no se anula todavía.
//! - Todo en una transacción: estado de la venta y movimientos de stock tipo 'devolucion'.

use sqlx::SqlitePool;

use crate::boleta::formato::folio;
use crate::error::validacion;
use crate::repositorios::{productos, turnos, ventas};
use crate::servicios::config;
use crate::servicios::historial::{self, DetalleVenta};
use crate::servicios::seguridad::{self, Accion, ControlPin};
use crate::servicios::ventas::{redondeo_a_decena, MedioPago};
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

/// Revisa el medio de reembolso según cómo se pagó: una fiada no se reembolsa (baja la deuda)
/// y una pagada necesita un medio que no sea fiado.
pub(crate) fn medio_valido(estado: &str, medio: Option<MedioPago>) -> Resultado<Option<MedioPago>> {
    match (estado, medio) {
        ("fiada", None) => Ok(None),
        ("fiada", Some(_)) => {
            validacion("Una venta fiada no se reembolsa: deja de contar como deuda")
        }
        (_, None) => validacion("Elige cómo se devuelve el dinero"),
        (_, Some(MedioPago::Fiado)) => validacion("El dinero no se puede devolver como fiado"),
        (_, Some(m)) => Ok(Some(m)),
    }
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
    if estado == "fiada" && v.tiene_abonos {
        return validacion("La venta fiada tiene abonos: todavía no se puede anular");
    }
    let medio = medio_valido(estado, medio_reembolso)?;
    let efectivo = medio == Some(MedioPago::Efectivo);
    // Antes del PIN, para no pedirlo en vano.
    if efectivo
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
    if efectivo && turno_id.is_none() {
        return Err(sin_caja());
    }
    // En efectivo se devuelve a la decena, como se cobra (una venta con débito de $1.224
    // anulada en efectivo devuelve $1.220).
    let reembolso = match medio {
        None => None,
        Some(MedioPago::Efectivo) if config::redondeo_efectivo_activo(&mut tx).await? => {
            Some(v.total + redondeo_a_decena(v.total))
        }
        Some(_) => Some(v.total),
    };
    let anulada = ventas::anular(
        &mut tx,
        venta_id,
        &ventas::DatosAnulacion {
            motivo: &motivo,
            turno_id,
            medio_reembolso: medio.map(MedioPago::como_texto),
            reembolso,
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
