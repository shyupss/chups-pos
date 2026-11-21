//! Reportes: la pantalla Hoy y los reportes por rango, en el día y la hora local del computador.

use chrono::Utc;
use pos_core::servicios::exportar_reportes::{self as exportar, TipoReporte};
use pos_core::servicios::reportes::{
    self as srv, Rango, ReporteFiados, ReporteGanancia, ReporteProductos, ReporteTurnos,
    ReporteVentas, ResumenHoy,
};
use pos_core::zona::ZonaLocal;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use super::{a_texto, ResultadoComando};
use crate::estado::EstadoApp;

fn rango(desde: &str, hasta: &str) -> ResultadoComando<Rango> {
    Rango::de_textos(desde, hasta).map_err(a_texto)
}

/// Todo lo de la pantalla Hoy en una llamada.
#[tauri::command]
pub async fn reporte_hoy(estado: State<'_, EstadoApp>) -> ResultadoComando<ResumenHoy> {
    srv::hoy(&estado.pool, &ZonaLocal, Utc::now())
        .await
        .map_err(a_texto)
}

#[tauri::command]
pub async fn reporte_ventas(
    estado: State<'_, EstadoApp>,
    desde: String,
    hasta: String,
) -> ResultadoComando<ReporteVentas> {
    srv::ventas(&estado.pool, &ZonaLocal, rango(&desde, &hasta)?)
        .await
        .map_err(a_texto)
}

/// `categoria`: null todas, 0 sin categoría.
#[tauri::command]
pub async fn reporte_productos(
    estado: State<'_, EstadoApp>,
    desde: String,
    hasta: String,
    categoria: Option<i64>,
) -> ResultadoComando<ReporteProductos> {
    srv::productos_vendidos(&estado.pool, &ZonaLocal, rango(&desde, &hasta)?, categoria)
        .await
        .map_err(a_texto)
}

#[tauri::command]
pub async fn reporte_ganancia(
    estado: State<'_, EstadoApp>,
    desde: String,
    hasta: String,
) -> ResultadoComando<ReporteGanancia> {
    srv::ganancia(&estado.pool, &ZonaLocal, rango(&desde, &hasta)?)
        .await
        .map_err(a_texto)
}

#[tauri::command]
pub async fn reporte_turnos(
    estado: State<'_, EstadoApp>,
    desde: String,
    hasta: String,
) -> ResultadoComando<ReporteTurnos> {
    srv::turnos(&estado.pool, &ZonaLocal, rango(&desde, &hasta)?)
        .await
        .map_err(a_texto)
}

#[tauri::command]
pub async fn reporte_fiados(
    estado: State<'_, EstadoApp>,
    desde: String,
    hasta: String,
) -> ResultadoComando<ReporteFiados> {
    srv::fiados(&estado.pool, &ZonaLocal, rango(&desde, &hasta)?, Utc::now())
        .await
        .map_err(a_texto)
}

/// Formato de una exportación.
#[derive(Debug, Clone, Copy, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Formato {
    Excel,
    Pdf,
}

impl Formato {
    fn extension(self) -> &'static str {
        match self {
            Formato::Excel => "xlsx",
            Formato::Pdf => "pdf",
        }
    }
}

/// Pregunta dónde guardar (Documentos\CHUPS POS\Reportes por defecto) y escribe el libro fuera
/// de los hilos async. `None` si se cancela el diálogo.
pub(crate) async fn guardar_libro(
    app: &AppHandle,
    estado: &EstadoApp,
    libro: pos_core::datos::Libro,
    nombre: String,
    formato: Formato,
) -> ResultadoComando<Option<String>> {
    let carpeta = estado
        .carpeta_pdf
        .parent()
        .map(|p| p.join("Reportes"))
        .unwrap_or_else(|| estado.carpeta_pdf.clone());
    let _ = std::fs::create_dir_all(&carpeta);
    let dialogo = app.clone();
    let ext = formato.extension();
    let elegida = tauri::async_runtime::spawn_blocking(move || {
        dialogo
            .dialog()
            .file()
            .set_title("Guardar como")
            .set_directory(carpeta)
            .set_file_name(format!("{nombre}.{ext}"))
            .add_filter(
                if formato == Formato::Excel {
                    "Excel"
                } else {
                    "PDF"
                },
                &[ext],
            )
            .blocking_save_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(ruta) = elegida.and_then(|r| r.into_path().ok()) else {
        return Ok(None);
    };
    let destino = ruta.clone();
    tauri::async_runtime::spawn_blocking(move || -> ResultadoComando<()> {
        match formato {
            Formato::Excel => {
                pos_core::datos::excel::escribir(&libro, &destino, &ZonaLocal).map_err(a_texto)
            }
            Formato::Pdf => {
                let bytes = pos_core::datos::pdf::generar(&libro, &ZonaLocal).map_err(a_texto)?;
                std::fs::write(&destino, bytes).map_err(|e| {
                    format!("No se pudo guardar el PDF (¿está abierto en otro programa?): {e}")
                })
            }
        }
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(Some(ruta.to_string_lossy().into_owned()))
}

/// Exporta un reporte por rango a Excel o PDF. Devuelve la ruta, o `None` si se canceló.
#[tauri::command]
pub async fn exportar_reporte(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
    tipo: TipoReporte,
    desde: String,
    hasta: String,
    categoria: Option<i64>,
    formato: Formato,
) -> ResultadoComando<Option<String>> {
    let (libro, nombre) = exportar::libro(
        &estado.pool,
        &ZonaLocal,
        tipo,
        rango(&desde, &hasta)?,
        categoria,
        Utc::now(),
    )
    .await
    .map_err(a_texto)?;
    guardar_libro(&app, &estado, libro, nombre, formato).await
}

/// Abre un archivo exportado con el programa del sistema (Excel, el visor de PDF).
#[tauri::command]
pub fn abrir_archivo(app: AppHandle, ruta: String) -> ResultadoComando<()> {
    app.opener()
        .open_path(ruta, None::<&str>)
        .map_err(|e| format!("No se pudo abrir el archivo: {e}"))
}
