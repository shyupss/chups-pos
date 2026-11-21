//! "CIERRE DE CAJA": el resumen del turno que se imprime al cerrar la caja, con las ventas por
//! medio de pago, lo anulado o devuelto, los ingresos y retiros, y el efectivo esperado contra
//! el contado.

use sqlx::SqlitePool;

use super::formato::{cantidad, fecha_local, folio, pesos, pesos_con_signo};
use super::texto::{self, Alineacion, Armado, Estilo, LineaTexto};
use super::{encabezado, Documento, Encabezado};
use crate::logo::LogoBn;
use crate::repositorios::productos::Unidad;
use crate::servicios::turnos::{self, ResumenTurno, TipoMovimiento};
use crate::servicios::ventas::MedioPago;
use crate::Resultado;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CierreCaja {
    pub negocio: Encabezado,
    pub resumen: ResumenTurno,
    /// Fecha y hora locales de apertura y cierre, "dd-mm-aaaa hh:mm".
    pub apertura: String,
    pub cierre: Option<String>,
}

pub async fn armar(pool: &SqlitePool, turno_id: i64) -> Resultado<CierreCaja> {
    let resumen = turnos::resumen(pool, turno_id).await?;
    let mut conn = pool.acquire().await?;
    Ok(CierreCaja {
        negocio: encabezado(&mut conn).await?,
        apertura: fecha_local(&resumen.turno.abierto_en),
        cierre: resumen.turno.cerrado_en.as_deref().map(fecha_local),
        resumen,
    })
}

pub fn renderizar(c: &CierreCaja, columnas: usize) -> Vec<LineaTexto> {
    use Alineacion::{Centro, Izquierda};
    let r = &c.resumen;
    let e = &r.efectivo;
    let mut a = Armado::nuevo(columnas);

    texto::encabezado(&mut a, &c.negocio);
    a.push(LineaTexto::Separador);
    a.texto("CIERRE DE CAJA", Centro, Estilo::TITULO);
    a.texto(
        &format!("Turno N° {}", folio(r.turno.id)),
        Centro,
        Estilo::NEGRITA,
    );
    a.push(LineaTexto::Separador);
    a.par("Apertura", &c.apertura, Estilo::NORMAL);
    a.par(
        "Cierre",
        c.cierre.as_deref().unwrap_or("caja abierta"),
        Estilo::NORMAL,
    );

    a.push(LineaTexto::Separador);
    a.texto("VENTAS", Izquierda, Estilo::NEGRITA);
    for t in &r.ventas {
        a.par(
            &format!("{} ({})", t.medio.nombre(), t.cantidad),
            &pesos(t.monto),
            Estilo::NORMAL,
        );
    }
    a.par(
        &format!("Total ({})", r.cantidad_ventas),
        &pesos(r.total_ventas),
        Estilo::NEGRITA,
    );
    if r.redondeo_ventas != 0 {
        a.par(
            "Redondeo del efectivo",
            &pesos_con_signo(r.redondeo_ventas),
            Estilo::NORMAL,
        );
    }

    if !r.anulaciones.is_empty() || !r.devoluciones.is_empty() {
        a.push(LineaTexto::Separador);
        // Por medio de reembolso. "Fiado (sin reembolso)": se anuló deuda, no salió dinero.
        for (titulo, lista) in [
            ("ANULACIONES", &r.anulaciones),
            ("DEVOLUCIONES", &r.devoluciones),
        ] {
            if lista.is_empty() {
                continue;
            }
            a.texto(titulo, Izquierda, Estilo::NEGRITA);
            for t in lista {
                let medio = match t.medio {
                    MedioPago::Fiado => "Fiado, sin reembolso",
                    m => m.nombre(),
                };
                a.par(
                    &format!("{medio} ({})", t.cantidad),
                    &pesos(-t.monto),
                    Estilo::NORMAL,
                );
            }
        }
        if r.redondeo_reembolsos != 0 {
            a.par(
                "Redondeo de reembolsos",
                &pesos_con_signo(r.redondeo_reembolsos),
                Estilo::NORMAL,
            );
        }
    }

    if !r.abonos.is_empty() {
        a.push(LineaTexto::Separador);
        a.texto("ABONOS DE FIADOS", Izquierda, Estilo::NEGRITA);
        for t in &r.abonos {
            a.par(
                &format!("{} ({})", t.medio.nombre(), t.cantidad),
                &pesos(t.monto),
                Estilo::NORMAL,
            );
        }
    }

    if !r.movimientos.is_empty() {
        a.push(LineaTexto::Separador);
        a.texto("INGRESOS Y RETIROS", Izquierda, Estilo::NEGRITA);
        for m in &r.movimientos {
            let (que, monto) = match m.tipo {
                TipoMovimiento::Ingreso => ("Ingreso", m.monto),
                TipoMovimiento::Retiro => ("Retiro", -m.monto),
            };
            a.par(
                &format!("{que}: {}", m.motivo),
                &pesos(monto),
                Estilo::NORMAL,
            );
        }
    }

    a.push(LineaTexto::Separador);
    a.texto("EFECTIVO", Izquierda, Estilo::NEGRITA);
    a.par("Monto inicial", &pesos(e.monto_inicial), Estilo::NORMAL);
    a.par("Ventas en efectivo", &pesos(e.ventas), Estilo::NORMAL);
    if e.abonos != 0 {
        a.par("Abonos en efectivo", &pesos(e.abonos), Estilo::NORMAL);
    }
    a.par("Ingresos", &pesos(e.ingresos), Estilo::NORMAL);
    a.par("Retiros", &pesos(-e.retiros), Estilo::NORMAL);
    a.par("Reembolsos", &pesos(-e.reembolsos), Estilo::NORMAL);
    a.par("Esperado", &pesos(e.esperado), Estilo::NEGRITA);
    if let (Some(contado), Some(diferencia)) = (r.turno.efectivo_contado, r.turno.diferencia) {
        a.par("Contado", &pesos(contado), Estilo::NEGRITA);
        a.par("DIFERENCIA", &pesos_con_signo(diferencia), Estilo::TOTAL);
        let veredicto = match diferencia {
            0 => "CAJA CUADRADA".to_owned(),
            d if d > 0 => format!("SOBRAN {}", pesos(d)),
            d => format!("FALTAN {}", pesos(-d)),
        };
        a.texto(&veredicto, Centro, Estilo::NEGRITA);
    }

    if let Some(conteo) = r.turno.detalle_conteo.as_ref().filter(|d| !d.is_empty()) {
        a.push(LineaTexto::Separador);
        a.texto("CONTEO", Izquierda, Estilo::NEGRITA);
        for d in conteo {
            a.par(
                &format!(
                    "{} x {}",
                    cantidad(d.cantidad * 1000, Unidad::Unidad),
                    pesos(d.denominacion)
                ),
                &pesos(d.cantidad * d.denominacion),
                Estilo::NORMAL,
            );
        }
    }

    if let Some(nota) = &r.turno.nota_cierre {
        a.push(LineaTexto::Separador);
        a.texto(&format!("Nota: {nota}"), Izquierda, Estilo::NORMAL);
    }

    // Firma de quien entrega la caja.
    a.push(LineaTexto::Separador);
    a.push(LineaTexto::Blanco);
    a.push(LineaTexto::Blanco);
    let raya = "_".repeat(a.columnas() * 2 / 3);
    a.texto(&raya, Centro, Estilo::NORMAL);
    a.texto("Firma", Centro, Estilo::NORMAL);
    a.lineas()
}

impl Documento for CierreCaja {
    fn lineas(&self, columnas: usize) -> Vec<LineaTexto> {
        renderizar(self, columnas)
    }
    fn logo(&self) -> Option<&LogoBn> {
        self.negocio.logo.as_ref()
    }
    fn titulo(&self) -> String {
        format!("Cierre de caja N° {}", folio(self.resumen.turno.id))
    }
    fn nombre(&self) -> String {
        format!("cierre-{}", folio(self.resumen.turno.id))
    }
}
