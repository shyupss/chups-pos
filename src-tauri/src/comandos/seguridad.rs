//! PIN de administrador. La verificación y el bloqueo viven en `pos-core`; aquí solo se pasa el
//! control de intentos del proceso.

use pos_core::servicios::config::AccionesProtegidas;
use pos_core::servicios::seguridad::{self as srv, EstadoSeguridad};
use tauri::State;

use super::{a_texto, ResultadoComando, ResultadoProtegido};
use crate::estado::EstadoApp;

#[tauri::command]
pub async fn estado_seguridad(estado: State<'_, EstadoApp>) -> ResultadoComando<EstadoSeguridad> {
    srv::estado(&estado.pool).await.map_err(a_texto)
}

/// Crea el PIN y devuelve el código de recuperación, que se muestra una sola vez.
#[tauri::command]
pub async fn crear_pin(estado: State<'_, EstadoApp>, pin: String) -> ResultadoComando<String> {
    srv::crear_pin(&estado.pool, &pin).await.map_err(a_texto)
}

#[tauri::command]
pub async fn cambiar_pin(
    estado: State<'_, EstadoApp>,
    actual: String,
    nuevo: String,
) -> ResultadoProtegido<()> {
    Ok(srv::cambiar_pin(&estado.pool, &estado.pin, &actual, &nuevo).await?)
}

#[tauri::command]
pub async fn quitar_pin(
    estado: State<'_, EstadoApp>,
    pin: Option<String>,
) -> ResultadoProtegido<()> {
    let Some(pin) = pin else {
        return Err(pos_core::Error::Pin(pos_core::FalloPin::Requerido).into());
    };
    Ok(srv::quitar_pin(&estado.pool, &estado.pin, &pin).await?)
}

/// PIN nuevo con el código de recuperación; devuelve el código nuevo.
#[tauri::command]
pub async fn recuperar_pin(
    estado: State<'_, EstadoApp>,
    codigo: String,
    nuevo: String,
) -> ResultadoProtegido<String> {
    Ok(srv::recuperar_pin(&estado.pool, &estado.pin, &codigo, &nuevo).await?)
}

#[tauri::command]
pub async fn guardar_protegidas(
    estado: State<'_, EstadoApp>,
    protegidas: AccionesProtegidas,
    pin: Option<String>,
) -> ResultadoProtegido<EstadoSeguridad> {
    Ok(srv::guardar_protegidas(&estado.pool, &estado.pin, protegidas, pin.as_deref()).await?)
}
