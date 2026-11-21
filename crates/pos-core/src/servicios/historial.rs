//! Historial de ventas: la lista con filtros (paginada, con totales calculados en SQL) y el
//! detalle de una venta con sus líneas, su anulación y sus devoluciones.

use chrono::{Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::validacion;
use crate::repositorios::devoluciones as repo_dev;
use crate::repositorios::productos::Unidad;
use crate::repositorios::ventas::{self as repo, EstadoBuscado, Filtro};
use crate::servicios::ventas::MedioPago;
use crate::{Error, Resultado};

/// Lo que se filtra desde la pantalla Ventas. Las fechas son días locales ("aaaa-mm-dd") y
/// `hasta` se incluye. Con folio, los demás filtros no se miran.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiltrosVentas {
    pub desde: Option<String>,
    pub hasta: Option<String>,
    pub folio: Option<i64>,
    pub medio_pago: Option<MedioPago>,
    pub estado: Option<EstadoFiltro>,
    pub turno_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EstadoFiltro {
    Pagada,
    Fiada,
    Anulada,
    /// Con alguna devolución, parcial o total.
    ConDevolucion,
}

/// Estado que se muestra en la insignia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EstadoVenta {
    Pagada,
    Fiada,
    Anulada,
    /// Se devolvió una parte.
    ConDevolucion,
    /// Se devolvió todo lo vendido.
    Devuelta,
}

pub(crate) fn estado_de(estado: &str, devolucion: Option<&str>) -> EstadoVenta {
    match (estado, devolucion) {
        ("anulada", _) => EstadoVenta::Anulada,
        (_, Some("total")) => EstadoVenta::Devuelta,
        (_, Some(_)) => EstadoVenta::ConDevolucion,
        ("fiada", None) => EstadoVenta::Fiada,
        _ => EstadoVenta::Pagada,
    }
}

fn medio(texto: &str) -> MedioPago {
    MedioPago::desde_texto(texto).unwrap_or(MedioPago::Efectivo)
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VentaListada {
    pub id: i64,
    pub folio: i64,
    pub fecha_hora: String,
    pub productos: i64,
    pub total: i64,
    pub medio_pago: MedioPago,
    pub estado: EstadoVenta,
}

/// Cantidad de ventas del filtro y lo vendido (sin anuladas, descontando lo devuelto).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TotalesVentas {
    pub cantidad: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PaginaVentas {
    pub ventas: Vec<VentaListada>,
    /// Solo en la primera página (desplazamiento 0): no cambian al bajar por la lista.
    pub totales: Option<TotalesVentas>,
}

/// Medianoche local de `dia` en UTC, como la guarda la base. Si esa medianoche no existe
/// (en Chile el cambio de hora de septiembre se salta las 00:00), la primera hora que sí.
pub(crate) fn inicio_del_dia(dia: NaiveDate) -> String {
    let medianoche: NaiveDateTime = dia.and_hms_opt(0, 0, 0).expect("00:00 es una hora válida");
    let local = (0..=2)
        .find_map(|h| {
            Local
                .from_local_datetime(&(medianoche + Duration::hours(h)))
                .earliest()
        })
        .unwrap_or_else(|| Local.from_utc_datetime(&medianoche));
    local
        .with_timezone(&Utc)
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

pub(crate) fn dia(texto: &str) -> Resultado<NaiveDate> {
    NaiveDate::parse_from_str(texto.trim(), "%Y-%m-%d")
        .map_err(|_| Error::Validacion(format!("La fecha «{texto}» no es válida")))
}

/// Traduce los filtros de la pantalla al filtro SQL (fechas locales a rango UTC).
fn a_filtro(f: &FiltrosVentas) -> Resultado<Filtro> {
    let desde = f.desde.as_deref().map(dia).transpose()?;
    let hasta = f.hasta.as_deref().map(dia).transpose()?;
    if let (Some(d), Some(h)) = (desde, hasta) {
        if d > h {
            return validacion("La fecha de inicio es posterior a la de término");
        }
    }
    Ok(Filtro {
        desde_utc: desde.map(inicio_del_dia),
        hasta_utc: hasta.and_then(|h| h.succ_opt()).map(inicio_del_dia),
        folio: f.folio,
        medio_pago: f.medio_pago.map(MedioPago::como_texto),
        estado: f.estado.map(|e| match e {
            EstadoFiltro::Pagada => EstadoBuscado::Pagada,
            EstadoFiltro::Fiada => EstadoBuscado::Fiada,
            EstadoFiltro::Anulada => EstadoBuscado::Anulada,
            EstadoFiltro::ConDevolucion => EstadoBuscado::ConDevolucion,
        }),
        turno_id: f.turno_id,
    })
}

/// Una página de ventas, de la más nueva a la más antigua.
pub async fn listar(
    pool: &SqlitePool,
    filtros: &FiltrosVentas,
    limite: i64,
    desplazamiento: i64,
) -> Resultado<PaginaVentas> {
    let f = a_filtro(filtros)?;
    let (limite, desplazamiento) = (limite.clamp(1, 500), desplazamiento.max(0));
    let ventas = repo::listar(pool, &f, limite, desplazamiento)
        .await?
        .into_iter()
        .map(|v| VentaListada {
            estado: estado_de(&v.estado, v.devolucion.as_deref()),
            medio_pago: medio(&v.medio_pago),
            id: v.id,
            folio: v.folio,
            fecha_hora: v.fecha_hora,
            productos: v.productos,
            total: v.total,
        })
        .collect();
    let totales = if desplazamiento == 0 {
        let (cantidad, total) = repo::totales(pool, &f).await?;
        Some(TotalesVentas { cantidad, total })
    } else {
        None
    };
    Ok(PaginaVentas { ventas, totales })
}

/// Id de la venta con ese folio (escrito o leído del código de barras de la boleta).
pub async fn por_folio(pool: &SqlitePool, folio: i64) -> Resultado<Option<i64>> {
    repo::id_por_folio(pool, folio).await
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LineaDetalle {
    /// Id de la línea (venta_items), el que se usa para devolver.
    pub id: i64,
    pub producto_id: i64,
    pub nombre: String,
    pub unidad: Unidad,
    pub precio_unitario: i64,
    pub cantidad_mil: i64,
    pub subtotal: i64,
    /// Lo ya devuelto de esta línea, en milésimas.
    pub devuelto_mil: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Anulacion {
    pub fecha: String,
    pub motivo: String,
    pub turno_id: Option<i64>,
    /// `None` en una venta fiada: no se devolvió dinero.
    pub medio_reembolso: Option<MedioPago>,
    pub reembolso: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LineaDevuelta {
    pub venta_item_id: i64,
    pub nombre: String,
    pub unidad: Unidad,
    pub precio_unitario: i64,
    pub cantidad_mil: i64,
    pub monto: i64,
    pub vuelve_a_stock: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Devolucion {
    pub id: i64,
    pub fecha: String,
    pub turno_id: Option<i64>,
    /// Lo devuelto al cliente, con el redondeo incluido.
    pub monto: i64,
    pub redondeo: i64,
    /// `None` en una venta fiada: bajó la deuda, no se devolvió dinero.
    pub medio_reembolso: Option<MedioPago>,
    pub motivo: String,
    pub lineas: Vec<LineaDevuelta>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DetalleVenta {
    pub id: i64,
    pub folio: i64,
    pub fecha_hora: String,
    pub subtotal: i64,
    pub descuento: i64,
    pub redondeo: i64,
    pub total: i64,
    pub medio_pago: MedioPago,
    pub monto_recibido: Option<i64>,
    pub vuelto: Option<i64>,
    pub cliente: Option<String>,
    pub estado: EstadoVenta,
    /// `None` en las ventas anteriores a los turnos.
    pub turno_id: Option<i64>,
    pub lineas: Vec<LineaDetalle>,
    pub anulacion: Option<Anulacion>,
    pub devoluciones: Vec<Devolucion>,
    pub monto_devuelto: i64,
    pub tiene_abonos: bool,
    /// Fiado: el cliente, lo abonado y lo que todavía debe de esta venta.
    pub cliente_id: Option<i64>,
    pub abonado: i64,
    pub saldo: Option<i64>,
}

pub async fn detalle(pool: &SqlitePool, id: i64) -> Resultado<DetalleVenta> {
    let mut conn = pool.acquire().await?;
    let v = repo::detalle(&mut conn, id)
        .await?
        .ok_or_else(|| Error::NoEncontrado("La venta no existe".into()))?;
    let lineas = repo::items_con_devuelto(&mut conn, id)
        .await?
        .into_iter()
        .map(|i| LineaDetalle {
            id: i.id,
            producto_id: i.producto_id,
            nombre: i.nombre,
            unidad: i.unidad,
            precio_unitario: i.precio_unitario,
            cantidad_mil: i.cantidad_mil,
            subtotal: i.subtotal,
            devuelto_mil: i.devuelto_mil,
        })
        .collect();
    let mut devueltas = repo_dev::items_de_venta(&mut conn, id).await?;
    let devoluciones = repo_dev::de_venta(&mut conn, id)
        .await?
        .into_iter()
        .map(|d| Devolucion {
            lineas: devueltas
                .extract_if(.., |l| l.devolucion_id == d.id)
                .map(|l| LineaDevuelta {
                    venta_item_id: l.venta_item_id,
                    nombre: l.nombre,
                    unidad: l.unidad,
                    precio_unitario: l.precio_unitario,
                    cantidad_mil: l.cantidad_mil,
                    monto: l.monto,
                    vuelve_a_stock: l.vuelve_a_stock,
                })
                .collect(),
            id: d.id,
            fecha: d.fecha,
            turno_id: d.turno_id,
            monto: d.monto,
            redondeo: d.redondeo,
            medio_reembolso: d.medio_reembolso.as_deref().map(medio),
            motivo: d.motivo,
        })
        .collect();
    let fiada = if v.medio_pago == "fiado" {
        crate::repositorios::fiados::venta(&mut conn, id).await?
    } else {
        None
    };
    let anulacion = v.anulada_en.clone().map(|fecha| Anulacion {
        fecha,
        motivo: v.motivo_anulacion.clone().unwrap_or_default(),
        turno_id: v.turno_anulacion_id,
        medio_reembolso: v.medio_reembolso_anulacion.as_deref().map(medio),
        reembolso: v.reembolso_anulacion,
    });
    Ok(DetalleVenta {
        estado: estado_de(&v.estado, v.devolucion.as_deref()),
        medio_pago: medio(&v.medio_pago),
        id: v.id,
        folio: v.folio,
        fecha_hora: v.fecha_hora,
        subtotal: v.subtotal,
        descuento: v.descuento,
        redondeo: v.redondeo,
        total: v.total,
        monto_recibido: v.monto_recibido,
        vuelto: v.vuelto,
        cliente: v.cliente,
        turno_id: v.turno_id,
        lineas,
        anulacion,
        devoluciones,
        monto_devuelto: v.monto_devuelto,
        tiene_abonos: v.abonos > 0,
        cliente_id: fiada.as_ref().and_then(|f| f.cliente_id),
        abonado: fiada.as_ref().map_or(0, |f| f.abonado),
        saldo: fiada
            .filter(|_| v.estado != "anulada")
            .map(|f| f.saldo.max(0)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estados_para_mostrar() {
        assert_eq!(estado_de("pagada", None), EstadoVenta::Pagada);
        assert_eq!(estado_de("fiada", None), EstadoVenta::Fiada);
        assert_eq!(estado_de("anulada", None), EstadoVenta::Anulada);
        assert_eq!(
            estado_de("pagada", Some("parcial")),
            EstadoVenta::ConDevolucion
        );
        assert_eq!(estado_de("fiada", Some("total")), EstadoVenta::Devuelta);
    }

    #[test]
    fn el_rango_cubre_el_dia_local_completo() {
        let f = a_filtro(&FiltrosVentas {
            desde: Some("2026-11-21".into()),
            hasta: Some("2026-11-21".into()),
            ..Default::default()
        })
        .unwrap();
        let (desde, hasta) = (f.desde_utc.unwrap(), f.hasta_utc.unwrap());
        // 24 horas entre una medianoche local y la siguiente (noviembre no tiene cambio de hora).
        let horas = |t: &str| chrono::DateTime::parse_from_rfc3339(t).unwrap();
        assert_eq!((horas(&hasta) - horas(&desde)).num_hours(), 24);
        assert!(desde.ends_with('Z') && desde.len() == "2026-11-21T03:00:00.000Z".len());

        assert!(a_filtro(&FiltrosVentas {
            desde: Some("2026-11-22".into()),
            hasta: Some("2026-11-21".into()),
            ..Default::default()
        })
        .is_err());
        assert!(a_filtro(&FiltrosVentas {
            desde: Some("21/11/2026".into()),
            ..Default::default()
        })
        .is_err());
    }
}
