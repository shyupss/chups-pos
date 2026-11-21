use pos_core::servicios::config::{self as srv, ConfigCaja};
use tauri::State;

use super::{a_texto, ResultadoComando};
use crate::estado::EstadoApp;

/// Ajustes que la caja necesita conocer (por ahora, si se redondea el efectivo).
#[tauri::command]
pub async fn config_caja(estado: State<'_, EstadoApp>) -> ResultadoComando<ConfigCaja> {
    srv::config_caja(&estado.pool).await.map_err(a_texto)
}

#[tauri::command]
pub async fn cambiar_redondeo_efectivo(
    estado: State<'_, EstadoApp>,
    activo: bool,
) -> ResultadoComando<()> {
    srv::cambiar_redondeo_efectivo(&estado.pool, activo)
        .await
        .map_err(a_texto)
}
