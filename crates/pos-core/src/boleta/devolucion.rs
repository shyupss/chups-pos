//! "NOTA DE DEVOLUCIÓN": lo que se imprime al devolver productos de una venta, con el folio de
//! la venta, las líneas devueltas y cuánto se reembolsó.

use sqlx::SqlitePool;

use super::formato::{detalle, fecha_local, folio, pesos, pesos_con_signo};
use super::texto::{self, Alineacion, Armado, Estilo, LineaTexto};
use super::{encabezado, Documento, Encabezado};
use crate::logo::LogoBn;
use crate::repositorios::productos::Unidad;
use crate::servicios::historial;
use crate::servicios::ventas::MedioPago;
use crate::{Error, Resultado};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineaNota {
    pub nombre: String,
    pub unidad: Unidad,
    pub cantidad_mil: i64,
    pub precio_unitario: i64,
    /// Su parte del monto (proporcional al subtotal vendido).
    pub monto: i64,
    pub vuelve_a_stock: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotaDevolucion {
    pub negocio: Encabezado,
    /// Número de la devolución.
    pub numero: i64,
    pub folio_venta: i64,
    /// Fechas locales "dd-mm-aaaa hh:mm".
    pub fecha: String,
    pub fecha_venta: String,
    pub cliente: Option<String>,
    pub lineas: Vec<LineaNota>,
    pub redondeo: i64,
    /// Lo devuelto, con el redondeo.
    pub monto: i64,
    /// `None` en una venta fiada: baja la deuda.
    pub medio_reembolso: Option<MedioPago>,
    pub motivo: String,
}

pub async fn armar(pool: &SqlitePool, devolucion_id: i64) -> Resultado<NotaDevolucion> {
    let venta_id = crate::servicios::devoluciones::venta_de(pool, devolucion_id).await?;
    let v = historial::detalle(pool, venta_id).await?;
    let d = v
        .devoluciones
        .into_iter()
        .find(|d| d.id == devolucion_id)
        .ok_or_else(|| Error::NoEncontrado("La devolución no existe".into()))?;
    let mut conn = pool.acquire().await?;
    Ok(NotaDevolucion {
        negocio: encabezado(&mut conn).await?,
        numero: d.id,
        folio_venta: v.folio,
        fecha: fecha_local(&d.fecha),
        fecha_venta: fecha_local(&v.fecha_hora),
        cliente: v.cliente,
        lineas: d
            .lineas
            .into_iter()
            .map(|l| LineaNota {
                nombre: l.nombre,
                unidad: l.unidad,
                cantidad_mil: l.cantidad_mil,
                precio_unitario: l.precio_unitario,
                monto: l.monto,
                vuelve_a_stock: l.vuelve_a_stock,
            })
            .collect(),
        redondeo: d.redondeo,
        monto: d.monto,
        medio_reembolso: d.medio_reembolso,
        motivo: d.motivo,
    })
}

pub fn renderizar(n: &NotaDevolucion, columnas: usize) -> Vec<LineaTexto> {
    use Alineacion::{Centro, Izquierda};
    let mut a = Armado::nuevo(columnas);
    texto::encabezado(&mut a, &n.negocio);

    a.push(LineaTexto::Separador);
    a.texto("NOTA DE DEVOLUCIÓN", Centro, Estilo::TOTAL);
    a.texto(
        &format!("Venta N° {}", folio(n.folio_venta)),
        Centro,
        Estilo::NEGRITA,
    );
    a.texto(&n.fecha, Centro, Estilo::NORMAL);
    a.push(LineaTexto::Separador);
    a.par("Venta del", &n.fecha_venta, Estilo::NORMAL);
    a.par("Devolución N°", &folio(n.numero), Estilo::NORMAL);
    if let Some(cliente) = &n.cliente {
        a.texto(&format!("Cliente: {cliente}"), Izquierda, Estilo::NORMAL);
    }

    a.push(LineaTexto::Separador);
    a.texto("DEVUELTO", Izquierda, Estilo::NEGRITA);
    for l in &n.lineas {
        a.texto(&l.nombre, Izquierda, Estilo::NORMAL);
        a.par(
            &detalle(l.cantidad_mil, l.unidad, l.precio_unitario),
            &pesos(l.monto),
            Estilo::NORMAL,
        );
        if !l.vuelve_a_stock {
            a.texto("  (no vuelve al stock: merma)", Izquierda, Estilo::NORMAL);
        }
    }

    a.push(LineaTexto::Separador);
    let subtotal: i64 = n.lineas.iter().map(|l| l.monto).sum();
    a.par("Subtotal", &pesos(subtotal), Estilo::NORMAL);
    // La parte del descuento de la venta, si tuvo.
    let descuento = subtotal - (n.monto - n.redondeo);
    if descuento != 0 {
        a.par("Descuento", &pesos(-descuento), Estilo::NORMAL);
    }
    if n.redondeo != 0 {
        a.par("Redondeo", &pesos_con_signo(n.redondeo), Estilo::NORMAL);
    }
    match n.medio_reembolso {
        Some(medio) => {
            a.par("TOTAL DEVUELTO", &pesos(n.monto), Estilo::TOTAL);
            a.par("Reembolso en", medio.nombre(), Estilo::NORMAL);
        }
        None => {
            a.par("BAJA LA DEUDA", &pesos(n.monto), Estilo::TOTAL);
            a.texto(
                "Venta fiada: no se devuelve dinero",
                Izquierda,
                Estilo::NORMAL,
            );
        }
    }

    a.push(LineaTexto::Separador);
    a.texto(&format!("Motivo: {}", n.motivo), Izquierda, Estilo::NORMAL);
    a.push(LineaTexto::Separador);
    a.push(LineaTexto::Blanco);
    a.push(LineaTexto::Blanco);
    let raya = "_".repeat(a.columnas() * 2 / 3);
    a.texto(&raya, Centro, Estilo::NORMAL);
    a.texto("Firma", Centro, Estilo::NORMAL);
    a.lineas()
}

impl Documento for NotaDevolucion {
    fn lineas(&self, columnas: usize) -> Vec<LineaTexto> {
        renderizar(self, columnas)
    }
    fn logo(&self) -> Option<&LogoBn> {
        self.negocio.logo.as_ref()
    }
    fn titulo(&self) -> String {
        format!("Nota de devolución N° {}", folio(self.numero))
    }
    fn nombre(&self) -> String {
        format!("devolucion-{}", folio(self.numero))
    }
}
