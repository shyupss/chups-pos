use pos_core::servicios::sistema::{self, EstadoSistema};
use tauri::State;

use super::{a_texto, ResultadoComando};
use crate::estado::EstadoApp;

#[tauri::command]
pub async fn estado_sistema(estado: State<'_, EstadoApp>) -> ResultadoComando<EstadoSistema> {
    sistema::estado(
        &estado.pool,
        env!("CARGO_PKG_VERSION"),
        &estado.ruta_db.to_string_lossy(),
        estado.ms_inicio,
    )
    .await
    .map_err(a_texto)
}
