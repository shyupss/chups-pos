//! Clientes a los que se les fía.

use pos_core::servicios::clientes::{self as srv, Cliente, DatosCliente};
use tauri::State;

use super::{a_texto, ResultadoComando};
use crate::estado::EstadoApp;

/// Clientes activos por nombre, teléfono o RUT, con su deuda y lo que les queda de crédito.
#[tauri::command]
pub async fn buscar_clientes(
    estado: State<'_, EstadoApp>,
    texto: String,
    limite: i64,
) -> ResultadoComando<Vec<Cliente>> {
    srv::buscar(&estado.pool, &texto, limite)
        .await
        .map_err(a_texto)
}

#[tauri::command]
pub async fn obtener_cliente(estado: State<'_, EstadoApp>, id: i64) -> ResultadoComando<Cliente> {
    srv::obtener(&estado.pool, id).await.map_err(a_texto)
}

#[tauri::command]
pub async fn crear_cliente(
    estado: State<'_, EstadoApp>,
    datos: DatosCliente,
) -> ResultadoComando<Cliente> {
    srv::crear(&estado.pool, &datos).await.map_err(a_texto)
}

#[tauri::command]
pub async fn editar_cliente(
    estado: State<'_, EstadoApp>,
    id: i64,
    datos: DatosCliente,
) -> ResultadoComando<Cliente> {
    srv::editar(&estado.pool, id, &datos).await.map_err(a_texto)
}

/// Desactivar solo si no debe nada.
#[tauri::command]
pub async fn cambiar_estado_cliente(
    estado: State<'_, EstadoApp>,
    id: i64,
    activo: bool,
) -> ResultadoComando<Cliente> {
    srv::cambiar_activo(&estado.pool, id, activo)
        .await
        .map_err(a_texto)
}
