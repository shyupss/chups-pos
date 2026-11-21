use pos_core::servicios::negocio::{self as srv, DatosNegocio, Negocio};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::{a_texto, ResultadoComando};
use crate::estado::EstadoApp;

#[tauri::command]
pub async fn obtener_negocio(estado: State<'_, EstadoApp>) -> ResultadoComando<Negocio> {
    srv::obtener(&estado.pool).await.map_err(a_texto)
}

#[tauri::command]
pub async fn guardar_negocio(
    estado: State<'_, EstadoApp>,
    datos: DatosNegocio,
) -> ResultadoComando<Negocio> {
    srv::guardar(&estado.pool, &datos).await.map_err(a_texto)
}

/// Abre el diálogo para elegir un PNG o JPG y lo deja como logo (escalado y en blanco y negro).
/// Devuelve `None` si el usuario cancela.
#[tauri::command]
pub async fn elegir_logo(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
) -> ResultadoComando<Option<Negocio>> {
    // El diálogo bloquea mientras está abierto: fuera de los hilos del runtime.
    let elegido = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Elegir el logo del negocio")
            .add_filter("Imagen PNG o JPG", &["png", "jpg", "jpeg"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(archivo) = elegido else {
        return Ok(None);
    };
    let ruta = archivo
        .into_path()
        .map_err(|_| "No se pudo abrir ese archivo".to_string())?;
    let bytes = tokio_fs_leer(ruta).await?;
    srv::guardar_logo(&estado.pool, &bytes)
        .await
        .map(Some)
        .map_err(a_texto)
}

#[tauri::command]
pub async fn quitar_logo(estado: State<'_, EstadoApp>) -> ResultadoComando<Negocio> {
    srv::quitar_logo(&estado.pool).await.map_err(a_texto)
}

async fn tokio_fs_leer(ruta: std::path::PathBuf) -> ResultadoComando<Vec<u8>> {
    tauri::async_runtime::spawn_blocking(move || std::fs::read(ruta))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("No se pudo leer la imagen: {e}"))
}
