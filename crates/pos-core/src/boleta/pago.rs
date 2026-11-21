//! "COMPROBANTE DE PAGO": lo que se imprime al recibir un pago de fiado, con las ventas que
//! salda y lo que el cliente sigue debiendo.

use sqlx::SqlitePool;

use super::formato::{fecha_local, folio, pesos, pesos_con_signo};
use super::texto::{self, Alineacion, Armado, Estilo, LineaTexto};
use super::{encabezado, Documento, Encabezado};
use crate::logo::LogoBn;
use crate::repositorios::fiados as repo;
use crate::servicios::ventas::MedioPago;
use crate::servicios::{clientes, fiados};
use crate::Resultado;

/// Una venta que el pago abonó: cuánto y cuánto le queda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VentaAbonada {
    pub folio: i64,
    pub abono: i64,
    /// 0 = saldada.
    pub saldo: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComprobantePago {
    pub negocio: Encabezado,
    pub numero: i64,
    /// Fecha local "dd-mm-aaaa hh:mm".
    pub fecha: String,
    pub cliente: String,
    pub monto: i64,
    pub redondeo: i64,
    pub medio: MedioPago,
    pub monto_recibido: Option<i64>,
    pub vuelto: Option<i64>,
    pub ventas: Vec<VentaAbonada>,
    pub deuda_restante: i64,
    /// Fecha de la anulación, si el pago se anuló.
    pub anulado: Option<String>,
}

pub async fn armar(pool: &SqlitePool, pago_id: i64) -> Resultado<ComprobantePago> {
    let p = fiados::pago(pool, pago_id).await?;
    let c = clientes::obtener(pool, p.cliente_id).await?;
    let mut conn = pool.acquire().await?;
    let mut ventas = Vec::with_capacity(p.abonos.len());
    for a in &p.abonos {
        let saldo = repo::venta(&mut conn, a.venta_id)
            .await?
            .map_or(0, |v| v.saldo.max(0));
        ventas.push(VentaAbonada {
            folio: a.folio,
            abono: a.monto,
            saldo,
        });
    }
    Ok(ComprobantePago {
        negocio: encabezado(&mut conn).await?,
        numero: p.id,
        fecha: fecha_local(&p.fecha),
        cliente: c.nombre,
        monto: p.monto,
        redondeo: p.redondeo,
        medio: p.medio_pago,
        monto_recibido: p.monto_recibido,
        vuelto: p.vuelto,
        ventas,
        deuda_restante: c.deuda,
        anulado: p.anulado_en.as_deref().map(fecha_local),
    })
}

pub fn renderizar(c: &ComprobantePago, columnas: usize) -> Vec<LineaTexto> {
    use Alineacion::{Centro, Izquierda};
    let mut a = Armado::nuevo(columnas);
    texto::encabezado(&mut a, &c.negocio);

    a.push(LineaTexto::Separador);
    a.texto("COMPROBANTE DE PAGO", Centro, Estilo::TOTAL);
    a.texto(&format!("N° {}", folio(c.numero)), Centro, Estilo::NEGRITA);
    a.texto(&c.fecha, Centro, Estilo::NORMAL);
    if let Some(cuando) = &c.anulado {
        a.texto("ANULADO", Centro, Estilo::TITULO);
        a.texto(&format!("Anulado el {cuando}"), Centro, Estilo::NORMAL);
    }
    a.push(LineaTexto::Separador);
    a.texto(
        &format!("Cliente: {}", c.cliente),
        Izquierda,
        Estilo::NEGRITA,
    );

    a.push(LineaTexto::Separador);
    a.texto("VENTAS QUE PAGA", Izquierda, Estilo::NEGRITA);
    for v in &c.ventas {
        let estado = if v.saldo == 0 {
            "saldada".to_owned()
        } else {
            format!("debe {}", pesos(v.saldo))
        };
        a.par(
            &format!("N° {} ({estado})", folio(v.folio)),
            &pesos(v.abono),
            Estilo::NORMAL,
        );
    }

    a.push(LineaTexto::Separador);
    a.par("PAGO", &pesos(c.monto), Estilo::TOTAL);
    a.par("Medio", c.medio.nombre(), Estilo::NORMAL);
    if c.medio == MedioPago::Efectivo {
        if c.redondeo != 0 {
            a.par("Redondeo", &pesos_con_signo(c.redondeo), Estilo::NORMAL);
            a.par("Pagó", &pesos(c.monto + c.redondeo), Estilo::NORMAL);
        }
        if let Some(r) = c.monto_recibido {
            a.par("Recibido", &pesos(r), Estilo::NORMAL);
        }
        if let Some(v) = c.vuelto {
            a.par("Vuelto", &pesos(v), Estilo::NEGRITA);
        }
    }
    a.push(LineaTexto::Separador);
    a.par("DEUDA RESTANTE", &pesos(c.deuda_restante), Estilo::TOTAL);
    if c.deuda_restante == 0 {
        a.texto("¡Cuenta al día!", Centro, Estilo::NEGRITA);
    }
    a.push(LineaTexto::Blanco);
    a.lineas()
}

impl Documento for ComprobantePago {
    fn lineas(&self, columnas: usize) -> Vec<LineaTexto> {
        renderizar(self, columnas)
    }
    fn logo(&self) -> Option<&LogoBn> {
        self.negocio.logo.as_ref()
    }
    fn titulo(&self) -> String {
        format!("Comprobante de pago N° {}", folio(self.numero))
    }
    fn nombre(&self) -> String {
        format!("pago-{}", folio(self.numero))
    }
}
