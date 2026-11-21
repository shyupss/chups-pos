//! La boleta: comprobante interno de cada venta (no es un documento tributario del SII).
//!
//! [`armar`] junta en una sola estructura todo lo que se imprime: negocio, venta y líneas.
//! El texto para la térmica ([`texto`] y [`escpos`]) y el PDF ([`pdf`]) salen de esa misma
//! estructura, así el papel y el PDF dicen exactamente lo mismo.

pub mod formato;
pub mod texto;

use sqlx::SqlitePool;

use crate::logo::{self, LogoBn};
use crate::repositorios::productos::Unidad;
use crate::repositorios::{negocio, ventas};
use crate::servicios::config;
use crate::servicios::ventas::MedioPago;
use crate::{Error, Resultado};

/// Encabezado con los datos del negocio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encabezado {
    pub nombre: String,
    pub rut: Option<String>,
    pub direccion: Option<String>,
    pub telefono: Option<String>,
    pub mensaje_pie: String,
    pub logo: Option<LogoBn>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineaBoleta {
    pub nombre: String,
    pub unidad: Unidad,
    pub cantidad_mil: i64,
    pub precio_unitario: i64,
    pub subtotal: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boleta {
    pub negocio: Encabezado,
    pub folio: i64,
    /// Fecha y hora locales del equipo, "dd-mm-aaaa hh:mm".
    pub fecha: String,
    pub lineas: Vec<LineaBoleta>,
    pub subtotal: i64,
    pub descuento: i64,
    pub redondeo: i64,
    pub total: i64,
    pub medio_pago: MedioPago,
    pub monto_recibido: Option<i64>,
    pub vuelto: Option<i64>,
    /// Nombre del cliente en una venta fiada.
    pub cliente: Option<String>,
    /// "Comprobante interno sin validez tributaria" al pie (config boleta_leyenda).
    pub leyenda: bool,
    /// Código de barras con el folio al pie (config boleta_codigo).
    pub codigo_barras: bool,
}

impl Boleta {
    /// Artículos de la venta: las unidades se cuentan una a una y cada pesada cuenta como uno.
    pub fn cantidad_productos(&self) -> i64 {
        self.lineas
            .iter()
            .map(|l| match l.unidad {
                Unidad::Unidad if l.cantidad_mil % 1000 == 0 => l.cantidad_mil / 1000,
                _ => 1,
            })
            .sum()
    }
}

/// Junta negocio, venta, líneas y configuración en una [`Boleta`].
pub async fn armar(pool: &SqlitePool, venta_id: i64) -> Resultado<Boleta> {
    let mut conn = pool.acquire().await?;
    let n = negocio::obtener(&mut conn).await?;
    if n.nombre.trim().is_empty() {
        return Err(Error::Validacion(
            "Falta el nombre del negocio: complétalo en Configuración > Negocio antes de imprimir"
                .into(),
        ));
    }
    let cfg = config::obtener_con(&mut conn).await?;
    let v = ventas::cabecera(&mut conn, venta_id)
        .await?
        .ok_or_else(|| Error::NoEncontrado("La venta no existe".into()))?;
    let lineas = ventas::items(&mut conn, venta_id)
        .await?
        .into_iter()
        .map(|i| LineaBoleta {
            nombre: i.nombre,
            unidad: i.unidad,
            cantidad_mil: i.cantidad_mil,
            precio_unitario: i.precio_unitario,
            subtotal: i.subtotal,
        })
        .collect();
    Ok(Boleta {
        negocio: Encabezado {
            nombre: n.nombre,
            rut: n.rut,
            direccion: n.direccion,
            telefono: n.telefono,
            mensaje_pie: n.mensaje_pie,
            // Un logo dañado no impide imprimir: la boleta sale sin él.
            logo: n
                .logo
                .as_deref()
                .and_then(|png| logo::decodificar(png).ok()),
        },
        folio: v.folio,
        fecha: formato::fecha_local(&v.fecha_hora),
        lineas,
        subtotal: v.subtotal,
        descuento: v.descuento,
        redondeo: v.redondeo,
        total: v.total,
        medio_pago: MedioPago::desde_texto(&v.medio_pago).unwrap_or(MedioPago::Efectivo),
        monto_recibido: v.monto_recibido,
        vuelto: v.vuelto,
        cliente: v.cliente,
        leyenda: cfg.boleta_leyenda,
        codigo_barras: cfg.boleta_codigo,
    })
}

/// Id de la última venta, para "Reimprimir" sin tener la venta en pantalla.
pub async fn ultima_venta(pool: &SqlitePool) -> Resultado<Option<i64>> {
    let mut conn = pool.acquire().await?;
    ventas::ultima(&mut conn).await
}
