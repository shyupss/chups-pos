//! Licencia: estado, activar con el texto o con el archivo `.chups-licencia`.
//!
//! Lo que la licencia permite lo deciden `abrir_turno` y `confirmar_venta` (ver `turnos.rs` y
//! `ventas.rs`), con `licencia::exigir_*`: la vista solo muestra el estado.

use pos_core::servicios::licencia::{self as srv, EstadoLicencia};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::{a_texto, ResultadoComando};
use crate::estado::EstadoApp;

#[tauri::command]
pub async fn estado_licencia(estado: State<'_, EstadoApp>) -> ResultadoComando<EstadoLicencia> {
    srv::estado(&estado.pool, &estado.licencia, srv::ahora())
        .await
        .map_err(a_texto)
}

/// Activa la licencia pegada (o dictada). Si viene sin equipo, queda atada a este.
#[tauri::command]
pub async fn activar_licencia(
    estado: State<'_, EstadoApp>,
    texto: String,
) -> ResultadoComando<EstadoLicencia> {
    srv::activar(&estado.pool, &estado.licencia, &texto, srv::ahora())
        .await
        .map_err(a_texto)
}

/// Elige el archivo `.chups-licencia` y lo activa. `None` si se canceló el diálogo.
#[tauri::command]
pub async fn cargar_archivo_licencia(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
) -> ResultadoComando<Option<EstadoLicencia>> {
    // El diálogo bloquea mientras está abierto: fuera de los hilos del runtime.
    let elegido = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Cargar la licencia de CHUPS POS")
            .add_filter("Licencia de CHUPS POS", &[pos_licencia::EXTENSION, "txt"])
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
    // Unos cientos de bytes: leerlo aquí no detiene nada.
    let texto = std::fs::read(&ruta).map_err(|e| format!("No se pudo leer el archivo: {e}"))?;
    // Una licencia son unos pocos cientos de caracteres: lo demás no es una licencia.
    if texto.len() > 16 * 1024 {
        return Err("Ese archivo no es una licencia de CHUPS POS".into());
    }
    let texto = String::from_utf8_lossy(&texto);
    srv::activar(&estado.pool, &estado.licencia, &texto, srv::ahora())
        .await
        .map(Some)
        .map_err(a_texto)
}
