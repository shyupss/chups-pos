//! Soporte: abrir la carpeta de registros y copiar la información para pedir ayuda.

use pos_core::servicios::soporte::{self, DatosSoporte};
use pos_core::servicios::{config, licencia, respaldos, sistema};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::{a_texto, ResultadoComando};
use crate::estado::EstadoApp;

/// "Windows 10 Pro 22H2 (compilación 19045)" desde el registro; en otros sistemas, su nombre.
#[cfg(windows)]
fn version_windows() -> String {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};
    use winreg::RegKey;
    let leer = || -> Option<String> {
        let k = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(
                r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
                KEY_READ | KEY_WOW64_64KEY,
            )
            .ok()?;
        let nombre: String = k.get_value("ProductName").ok()?;
        let version: String = k.get_value("DisplayVersion").unwrap_or_default();
        let build: String = k.get_value("CurrentBuildNumber").unwrap_or_default();
        // El registro sigue diciendo "Windows 10" en Windows 11: desde la compilación 22000 es el 11.
        let nombre = if build.parse::<u32>().is_ok_and(|b| b >= 22000) {
            nombre.replace("Windows 10", "Windows 11")
        } else {
            nombre
        };
        Some(format!("{nombre} {version} (compilación {build})").replace("  ", " "))
    };
    leer().unwrap_or_else(|| "Windows (versión desconocida)".to_owned())
}

#[cfg(not(windows))]
fn version_windows() -> String {
    std::env::consts::OS.to_owned()
}

/// Abre la carpeta `Registros` en el Explorador.
#[tauri::command]
pub async fn abrir_carpeta_registros(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
) -> ResultadoComando<()> {
    let carpeta = estado.carpeta_datos.join(crate::registro::CARPETA);
    let _ = std::fs::create_dir_all(&carpeta);
    app.opener()
        .open_path(carpeta.to_string_lossy(), None::<&str>)
        .map_err(|e| format!("No se pudo abrir la carpeta: {e}"))
}

/// El texto que el usuario copia y manda al soporte: versión, Windows, estado de la licencia (sin la
/// clave), versión del esquema, impresora y carpeta de datos. Sin datos de clientes ni el PIN.
#[tauri::command]
pub async fn informacion_soporte(estado: State<'_, EstadoApp>) -> ResultadoComando<String> {
    let cfg = config::obtener(&estado.pool).await.map_err(a_texto)?;
    let sis = sistema::estado(
        &estado.pool,
        env!("CARGO_PKG_VERSION"),
        &estado.ruta_db.to_string_lossy(),
        estado.ms_inicio,
    )
    .await
    .map_err(a_texto)?;
    let lic = licencia::estado(&estado.pool, &estado.licencia, licencia::ahora())
        .await
        .map_err(a_texto)?;
    Ok(soporte::texto(&DatosSoporte {
        version_app: env!("CARGO_PKG_VERSION").to_owned(),
        windows: version_windows(),
        version_esquema: sis.version_esquema,
        impresora: cfg.impresora,
        ruta_datos: estado.carpeta_datos.to_string_lossy().into_owned(),
        tamano_base_bytes: respaldos::tamano_base(&estado.ruta_db),
        licencia: lic,
    }))
}
