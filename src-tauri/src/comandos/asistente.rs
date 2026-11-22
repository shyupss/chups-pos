//! Asistente de primer arranque: si hay que mostrarlo y marcarlo como terminado.

use pos_core::servicios::primer_arranque as srv;
use tauri::State;

use super::{a_texto, ResultadoComando};
use crate::estado::EstadoApp;

/// `true` si la base está vacía y el asistente todavía no se terminó.
#[tauri::command]
pub async fn asistente_pendiente(estado: State<'_, EstadoApp>) -> ResultadoComando<bool> {
    srv::pendiente(&estado.pool).await.map_err(a_texto)
}

/// Al terminar (o saltar) el asistente: no vuelve a aparecer solo.
#[tauri::command]
pub async fn terminar_asistente(estado: State<'_, EstadoApp>) -> ResultadoComando<()> {
    srv::terminar(&estado.pool).await.map_err(a_texto)
}
