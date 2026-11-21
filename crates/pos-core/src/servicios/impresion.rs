//! Salidas de la boleta según la configuración: PDF y bytes para la impresora térmica.
//! No toca hardware ni hilos: el controlador genera en segundo plano y manda los bytes.

use sqlx::SqlitePool;

use crate::boleta::{self, pdf, Boleta};
use crate::servicios::config::{self, FormatoPdf};
use crate::Resultado;

/// Todo lo necesario para dibujar el PDF de una venta, ya leído de la base.
pub struct PdfPendiente {
    pub boleta: Boleta,
    pub formato: FormatoPdf,
    /// Carpeta elegida en Configuración; `None` usa Documentos\CHUPS POS\Boletas.
    pub carpeta: Option<String>,
}

impl PdfPendiente {
    /// boleta-000123.pdf
    pub fn nombre_archivo(&self) -> String {
        pdf::nombre_archivo(self.boleta.folio)
    }

    /// Dibuja el PDF. Es trabajo de CPU: conviene llamarlo fuera de los hilos async.
    pub fn generar(&self) -> Resultado<Vec<u8>> {
        pdf::boleta(&self.boleta, self.formato)
    }
}

pub async fn preparar_pdf(pool: &SqlitePool, venta_id: i64) -> Resultado<PdfPendiente> {
    let boleta = boleta::armar(pool, venta_id).await?;
    let cfg = config::obtener(pool).await?;
    Ok(PdfPendiente {
        boleta,
        formato: cfg.pdf_formato,
        carpeta: cfg.pdf_carpeta,
    })
}
