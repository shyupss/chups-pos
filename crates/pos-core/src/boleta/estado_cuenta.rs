//! "ESTADO DE CUENTA": el detalle de lo que debe un cliente, para imprimir o guardar en PDF.

use chrono::Local;
use sqlx::SqlitePool;

use super::formato::{fecha_local, folio, pesos};
use super::texto::{self, Alineacion, Armado, Estilo, LineaTexto};
use super::{encabezado, Documento, Encabezado};
use crate::logo::LogoBn;
use crate::servicios::fiados;
use crate::servicios::ventas::MedioPago;
use crate::Resultado;

/// Una venta fiada pendiente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pendiente {
    pub folio: i64,
    /// Fecha local "dd-mm-aaaa hh:mm".
    pub fecha: String,
    pub total: i64,
    /// Lo que ya bajó (abonos y devoluciones).
    pub descontado: i64,
    pub saldo: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagoReciente {
    pub fecha: String,
    pub monto: i64,
    pub medio: MedioPago,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstadoCuenta {
    pub negocio: Encabezado,
    pub id_cliente: i64,
    pub cliente: String,
    pub rut: Option<String>,
    pub telefono: Option<String>,
    /// Fecha local de emisión.
    pub fecha: String,
    pub pendientes: Vec<Pendiente>,
    pub deuda: i64,
    pub limite: Option<i64>,
    /// Los últimos pagos vigentes (hasta 5).
    pub pagos: Vec<PagoReciente>,
}

pub async fn armar(pool: &SqlitePool, cliente_id: i64) -> Resultado<EstadoCuenta> {
    let f = fiados::ficha(pool, cliente_id).await?;
    let mut conn = pool.acquire().await?;
    Ok(EstadoCuenta {
        negocio: encabezado(&mut conn).await?,
        id_cliente: f.cliente.id,
        cliente: f.cliente.nombre,
        rut: f.cliente.rut,
        telefono: f.cliente.telefono,
        fecha: Local::now().format("%d-%m-%Y %H:%M").to_string(),
        pendientes: f
            .pendientes
            .into_iter()
            .map(|v| Pendiente {
                folio: v.folio,
                fecha: fecha_local(&v.fecha_hora),
                total: v.total,
                descontado: v.total - v.saldo,
                saldo: v.saldo,
            })
            .collect(),
        deuda: f.cliente.deuda,
        limite: f.cliente.limite_credito,
        pagos: f
            .pagos
            .into_iter()
            .filter(|p| p.anulado_en.is_none())
            .take(5)
            .map(|p| PagoReciente {
                fecha: fecha_local(&p.fecha),
                monto: p.monto,
                medio: p.medio_pago,
            })
            .collect(),
    })
}

pub fn renderizar(e: &EstadoCuenta, columnas: usize) -> Vec<LineaTexto> {
    use Alineacion::{Centro, Izquierda};
    let mut a = Armado::nuevo(columnas);
    texto::encabezado(&mut a, &e.negocio);

    a.push(LineaTexto::Separador);
    a.texto("ESTADO DE CUENTA", Centro, Estilo::TOTAL);
    a.texto(&e.fecha, Centro, Estilo::NORMAL);
    a.push(LineaTexto::Separador);
    a.texto(
        &format!("Cliente: {}", e.cliente),
        Izquierda,
        Estilo::NEGRITA,
    );
    if let Some(rut) = &e.rut {
        a.texto(&format!("RUT {rut}"), Izquierda, Estilo::NORMAL);
    }
    if let Some(t) = &e.telefono {
        a.texto(&format!("Tel. {t}"), Izquierda, Estilo::NORMAL);
    }

    a.push(LineaTexto::Separador);
    if e.pendientes.is_empty() {
        a.texto("No tiene ventas pendientes.", Izquierda, Estilo::NORMAL);
    } else {
        a.texto("VENTAS PENDIENTES", Izquierda, Estilo::NEGRITA);
        for v in &e.pendientes {
            a.par(
                &format!(
                    "N° {}  {}",
                    folio(v.folio),
                    &v.fecha[..10.min(v.fecha.len())]
                ),
                &pesos(v.saldo),
                Estilo::NORMAL,
            );
            if v.descontado > 0 {
                a.texto(
                    &format!(
                        "  Total {}, ya pagado {}",
                        pesos(v.total),
                        pesos(v.descontado)
                    ),
                    Izquierda,
                    Estilo::NORMAL,
                );
            }
        }
    }
    a.push(LineaTexto::Separador);
    a.par("DEUDA TOTAL", &pesos(e.deuda), Estilo::TOTAL);
    if let Some(l) = e.limite {
        a.par("Límite de crédito", &pesos(l), Estilo::NORMAL);
        a.par("Disponible", &pesos((l - e.deuda).max(0)), Estilo::NORMAL);
    }

    if !e.pagos.is_empty() {
        a.push(LineaTexto::Separador);
        a.texto("ÚLTIMOS PAGOS", Izquierda, Estilo::NEGRITA);
        for p in &e.pagos {
            a.par(
                &format!(
                    "{}  {}",
                    &p.fecha[..10.min(p.fecha.len())],
                    p.medio.nombre()
                ),
                &pesos(p.monto),
                Estilo::NORMAL,
            );
        }
    }
    a.push(LineaTexto::Blanco);
    a.lineas()
}

impl Documento for EstadoCuenta {
    fn lineas(&self, columnas: usize) -> Vec<LineaTexto> {
        renderizar(self, columnas)
    }
    fn logo(&self) -> Option<&LogoBn> {
        self.negocio.logo.as_ref()
    }
    fn titulo(&self) -> String {
        format!("Estado de cuenta de {}", self.cliente)
    }
    fn nombre(&self) -> String {
        format!("estado-cuenta-{:06}", self.id_cliente)
    }
}
