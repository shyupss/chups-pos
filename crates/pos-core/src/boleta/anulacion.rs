//! "ANULACIÓN DE VENTA N° 000123": el comprobante que se imprime al anular una venta, con lo que
//! volvió al stock y cuánto se devolvió y cómo.

use sqlx::SqlitePool;

use super::formato::{detalle, fecha_local, folio, pesos, pesos_con_signo};
use super::texto::{self, Alineacion, Armado, Estilo, LineaTexto};
use super::{encabezado, Documento, Encabezado, LineaBoleta};
use crate::logo::LogoBn;
use crate::servicios::historial;
use crate::servicios::ventas::MedioPago;
use crate::{Error, Resultado};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComprobanteAnulacion {
    pub negocio: Encabezado,
    pub folio: i64,
    /// Fechas locales "dd-mm-aaaa hh:mm".
    pub fecha_venta: String,
    pub fecha_anulacion: String,
    pub lineas: Vec<LineaBoleta>,
    pub total: i64,
    pub medio_pago: MedioPago,
    pub cliente: Option<String>,
    /// `None` en una venta fiada: no se devolvió dinero, se anuló la deuda.
    pub medio_reembolso: Option<MedioPago>,
    pub reembolso: Option<i64>,
    pub motivo: String,
}

pub async fn armar(pool: &SqlitePool, venta_id: i64) -> Resultado<ComprobanteAnulacion> {
    let v = historial::detalle(pool, venta_id).await?;
    let a = v
        .anulacion
        .ok_or_else(|| Error::Validacion("La venta no está anulada".into()))?;
    let mut conn = pool.acquire().await?;
    Ok(ComprobanteAnulacion {
        negocio: encabezado(&mut conn).await?,
        folio: v.folio,
        fecha_venta: fecha_local(&v.fecha_hora),
        fecha_anulacion: fecha_local(&a.fecha),
        lineas: v
            .lineas
            .into_iter()
            .map(|l| LineaBoleta {
                nombre: l.nombre,
                unidad: l.unidad,
                cantidad_mil: l.cantidad_mil,
                precio_unitario: l.precio_unitario,
                subtotal: l.subtotal,
            })
            .collect(),
        total: v.total,
        medio_pago: v.medio_pago,
        cliente: v.cliente,
        medio_reembolso: a.medio_reembolso,
        reembolso: a.reembolso,
        motivo: a.motivo,
    })
}

pub fn renderizar(c: &ComprobanteAnulacion, columnas: usize) -> Vec<LineaTexto> {
    use Alineacion::{Centro, Izquierda};
    let mut a = Armado::nuevo(columnas);
    texto::encabezado(&mut a, &c.negocio);

    a.push(LineaTexto::Separador);
    a.texto(
        &format!("ANULACIÓN DE VENTA N° {}", folio(c.folio)),
        Centro,
        Estilo::TOTAL,
    );
    a.texto(&c.fecha_anulacion, Centro, Estilo::NORMAL);
    a.push(LineaTexto::Separador);
    a.par("Venta del", &c.fecha_venta, Estilo::NORMAL);
    a.par("Pago", c.medio_pago.nombre(), Estilo::NORMAL);
    if let Some(cliente) = &c.cliente {
        a.texto(&format!("Cliente: {cliente}"), Izquierda, Estilo::NORMAL);
    }

    a.push(LineaTexto::Separador);
    a.texto("VUELVE AL STOCK", Izquierda, Estilo::NEGRITA);
    for l in &c.lineas {
        a.texto(&l.nombre, Izquierda, Estilo::NORMAL);
        a.par(
            &detalle(l.cantidad_mil, l.unidad, l.precio_unitario),
            &pesos(l.subtotal),
            Estilo::NORMAL,
        );
    }

    a.push(LineaTexto::Separador);
    a.par("Total de la venta", &pesos(c.total), Estilo::NORMAL);
    match (c.medio_reembolso, c.reembolso) {
        (Some(medio), Some(monto)) => {
            if monto != c.total {
                a.par(
                    "Redondeo",
                    &pesos_con_signo(monto - c.total),
                    Estilo::NORMAL,
                );
            }
            a.par("REEMBOLSO", &pesos(monto), Estilo::TOTAL);
            a.par("Se devolvió en", medio.nombre(), Estilo::NORMAL);
        }
        _ => {
            a.par("DEUDA ANULADA", &pesos(c.total), Estilo::TOTAL);
            a.texto(
                "Venta fiada: no se devuelve dinero",
                Izquierda,
                Estilo::NORMAL,
            );
        }
    }

    a.push(LineaTexto::Separador);
    a.texto(&format!("Motivo: {}", c.motivo), Izquierda, Estilo::NORMAL);
    a.push(LineaTexto::Separador);
    a.push(LineaTexto::Blanco);
    a.push(LineaTexto::Blanco);
    let raya = "_".repeat(a.columnas() * 2 / 3);
    a.texto(&raya, Centro, Estilo::NORMAL);
    a.texto("Firma", Centro, Estilo::NORMAL);
    a.lineas()
}

impl Documento for ComprobanteAnulacion {
    fn lineas(&self, columnas: usize) -> Vec<LineaTexto> {
        renderizar(self, columnas)
    }
    fn logo(&self) -> Option<&LogoBn> {
        self.negocio.logo.as_ref()
    }
    fn titulo(&self) -> String {
        format!("Anulación de venta N° {}", folio(self.folio))
    }
    fn nombre(&self) -> String {
        format!("anulacion-{}", folio(self.folio))
    }
}
