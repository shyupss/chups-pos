//! Salidas de la boleta según la configuración: PDF y bytes para la impresora térmica.
//! No toca hardware ni hilos: el controlador genera en segundo plano y manda los bytes.

use sqlx::SqlitePool;

use crate::boleta::{self, escpos, pdf, Boleta};
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

/// Bytes ESC/POS listos para la impresora elegida en Configuración.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trabajo {
    pub impresora: String,
    /// Nombre en la cola de impresión (y del .bin en la impresora de pruebas).
    pub documento: String,
    pub bytes: Vec<u8>,
}

fn impresora(cfg: &config::Config) -> Resultado<String> {
    cfg.impresora.clone().ok_or_else(|| {
        crate::Error::Validacion(
            "No hay impresora elegida: elígela en Configuración > Impresora y escáner".into(),
        )
    })
}

fn opciones(cfg: &config::Config) -> escpos::Opciones {
    escpos::Opciones {
        columnas: cfg.columnas(),
        tabla: cfg.tabla_caracteres,
    }
}

/// La boleta de una venta; sin `copias`, las de la configuración.
pub async fn trabajo_boleta(
    pool: &SqlitePool,
    venta_id: i64,
    copias: Option<u8>,
) -> Resultado<Trabajo> {
    let b = boleta::armar(pool, venta_id).await?;
    let cfg = config::obtener(pool).await?;
    let copias = copias.unwrap_or(cfg.copias).clamp(1, 3);
    Ok(Trabajo {
        impresora: impresora(&cfg)?,
        documento: format!("boleta-{}", boleta::formato::folio(b.folio)),
        bytes: escpos::copias(&escpos::boleta(&b, &opciones(&cfg)), copias),
    })
}

/// Página de prueba con la configuración actual (ancho del papel y tabla de caracteres).
pub async fn trabajo_prueba(pool: &SqlitePool) -> Resultado<Trabajo> {
    let cfg = config::obtener(pool).await?;
    let mut conn = pool.acquire().await?;
    let nombre = crate::repositorios::negocio::obtener(&mut conn)
        .await?
        .nombre;
    Ok(Trabajo {
        impresora: impresora(&cfg)?,
        documento: "pagina-de-prueba".into(),
        bytes: escpos::pagina_prueba(&nombre, &opciones(&cfg)),
    })
}

/// Pulso para abrir el cajón con el pin y la duración configurados.
pub async fn trabajo_cajon(pool: &SqlitePool) -> Resultado<Trabajo> {
    let cfg = config::obtener(pool).await?;
    Ok(Trabajo {
        impresora: impresora(&cfg)?,
        documento: "abrir-cajon".into(),
        bytes: escpos::pulso_cajon(cfg.cajon_pin, cfg.cajon_pulso_ms),
    })
}
