use pos_core::servicios::config::{self as srv, Config};
use tauri::State;

use super::{a_texto, ResultadoComando};
use crate::estado::EstadoApp;

/// Toda la configuración tipada (caja, boleta, impresora, cajón y balanza).
#[tauri::command]
pub async fn obtener_config(estado: State<'_, EstadoApp>) -> ResultadoComando<Config> {
    srv::obtener(&estado.pool).await.map_err(a_texto)
}

/// Valida y guarda la configuración completa; devuelve la que quedó.
#[tauri::command]
pub async fn guardar_config(
    estado: State<'_, EstadoApp>,
    config: Config,
) -> ResultadoComando<Config> {
    srv::guardar(&estado.pool, &config).await.map_err(a_texto)
}
