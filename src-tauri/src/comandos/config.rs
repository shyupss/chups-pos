use pos_core::servicios::config::{self as srv, Config};
use tauri::{State, WebviewWindow};

use super::{a_texto, ResultadoComando, ResultadoProtegido};
use crate::estado::EstadoApp;

/// Toda la configuración tipada (caja, boleta, impresora, cajón y balanza).
#[tauri::command]
pub async fn obtener_config(estado: State<'_, EstadoApp>) -> ResultadoComando<Config> {
    srv::obtener(&estado.pool).await.map_err(a_texto)
}

/// Valida y guarda la configuración completa; devuelve la que quedó.
/// Pide el PIN si "cambiar configuración" está protegida (menos para el tamaño de la interfaz).
/// El tamaño se aplica al instante como zoom del webview, sin reiniciar.
#[tauri::command]
pub async fn guardar_config(
    estado: State<'_, EstadoApp>,
    ventana: WebviewWindow,
    config: Config,
    pin: Option<String>,
) -> ResultadoProtegido<Config> {
    let guardada = srv::guardar(&estado.pool, &estado.pin, pin.as_deref(), &config).await?;
    crate::ventana::aplicar_escala(&ventana, guardada.escala_interfaz);
    Ok(guardada)
}
