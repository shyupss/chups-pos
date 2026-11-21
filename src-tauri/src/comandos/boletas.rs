use std::path::PathBuf;

use pos_core::servicios::impresion as srv;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use super::{a_texto, ResultadoComando};
use crate::estado::EstadoApp;

/// Guarda el PDF de la boleta (Documentos\CHUPS POS\Boletas o la carpeta configurada) y,
/// si se pide, lo abre con el visor del sistema. Devuelve la ruta del archivo.
#[tauri::command]
pub async fn guardar_boleta_pdf(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
    venta_id: i64,
    abrir: bool,
) -> ResultadoComando<String> {
    let pendiente = srv::preparar_pdf(&estado.pool, venta_id)
        .await
        .map_err(a_texto)?;
    let carpeta = pendiente
        .carpeta
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| estado.carpeta_pdf.clone());
    let ruta = carpeta.join(pendiente.nombre_archivo());
    let destino = ruta.clone();
    // Dibujar y escribir el PDF fuera de los hilos async.
    tauri::async_runtime::spawn_blocking(move || -> ResultadoComando<()> {
        let bytes = pendiente.generar().map_err(a_texto)?;
        std::fs::create_dir_all(&carpeta)
            .map_err(|e| format!("No se pudo crear la carpeta {}: {e}", carpeta.display()))?;
        std::fs::write(&destino, bytes).map_err(|e| {
            format!("No se pudo guardar el PDF (¿está abierto en otro programa?): {e}")
        })
    })
    .await
    .map_err(|e| e.to_string())??;
    let ruta = ruta.to_string_lossy().into_owned();
    if abrir {
        app.opener()
            .open_path(ruta.clone(), None::<&str>)
            .map_err(|e| format!("El PDF se guardó en {ruta}, pero no se pudo abrir: {e}"))?;
    }
    Ok(ruta)
}

/// Carpeta donde se guardan los PDF si no se eligió otra.
#[tauri::command]
pub fn carpeta_pdf_predeterminada(estado: State<'_, EstadoApp>) -> String {
    estado.carpeta_pdf.to_string_lossy().into_owned()
}

/// Diálogo para elegir la carpeta de los PDF. `None` si el usuario cancela.
#[tauri::command]
pub async fn elegir_carpeta_pdf(app: AppHandle) -> ResultadoComando<Option<String>> {
    let elegida = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Carpeta para los PDF de las boletas")
            .blocking_pick_folder()
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(elegida
        .and_then(|c| c.into_path().ok())
        .map(|c| c.to_string_lossy().into_owned()))
}

/// Id de la última venta (para Ctrl+P "reimprimir la última boleta" al abrir la app).
#[tauri::command]
pub async fn ultima_venta(estado: State<'_, EstadoApp>) -> ResultadoComando<Option<i64>> {
    pos_core::boleta::ultima_venta(&estado.pool)
        .await
        .map_err(a_texto)
}
