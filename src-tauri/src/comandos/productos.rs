use pos_core::repositorios::productos::Producto;
use pos_core::servicios::productos::{self as srv, DatosProducto};
use tauri::State;

use super::{a_texto, ResultadoComando};
use crate::estado::EstadoApp;

/// Escaneo: busca un producto activo por código exacto.
#[tauri::command]
pub async fn buscar_producto_por_codigo(
    estado: State<'_, EstadoApp>,
    codigo: String,
) -> ResultadoComando<Option<Producto>> {
    srv::buscar_por_codigo(&estado.pool, &codigo)
        .await
        .map_err(a_texto)
}

#[tauri::command]
pub async fn buscar_productos(
    estado: State<'_, EstadoApp>,
    texto: String,
    incluir_inactivos: bool,
    limite: i64,
    desplazamiento: i64,
) -> ResultadoComando<Vec<Producto>> {
    srv::buscar(
        &estado.pool,
        &texto,
        incluir_inactivos,
        limite,
        desplazamiento,
    )
    .await
    .map_err(a_texto)
}

#[tauri::command]
pub async fn crear_producto(
    estado: State<'_, EstadoApp>,
    datos: DatosProducto,
) -> ResultadoComando<Producto> {
    srv::crear(&estado.pool, &datos).await.map_err(a_texto)
}

#[tauri::command]
pub async fn editar_producto(
    estado: State<'_, EstadoApp>,
    id: i64,
    datos: DatosProducto,
) -> ResultadoComando<Producto> {
    srv::editar(&estado.pool, id, &datos).await.map_err(a_texto)
}

#[tauri::command]
pub async fn cambiar_estado_producto(
    estado: State<'_, EstadoApp>,
    id: i64,
    activo: bool,
) -> ResultadoComando<()> {
    srv::cambiar_activo(&estado.pool, id, activo)
        .await
        .map_err(a_texto)
}
