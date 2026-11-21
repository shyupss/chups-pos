use pos_core::repositorios::categorias::Categoria;
use pos_core::servicios::categorias as srv;
use tauri::State;

use super::{a_texto, ResultadoComando};
use crate::estado::EstadoApp;

/// Categorías por nombre, con la cantidad de productos activos de cada una.
#[tauri::command]
pub async fn listar_categorias(estado: State<'_, EstadoApp>) -> ResultadoComando<Vec<Categoria>> {
    srv::listar(&estado.pool).await.map_err(a_texto)
}

#[tauri::command]
pub async fn crear_categoria(
    estado: State<'_, EstadoApp>,
    nombre: String,
) -> ResultadoComando<Categoria> {
    srv::crear(&estado.pool, &nombre).await.map_err(a_texto)
}

#[tauri::command]
pub async fn renombrar_categoria(
    estado: State<'_, EstadoApp>,
    id: i64,
    nombre: String,
) -> ResultadoComando<Categoria> {
    srv::renombrar(&estado.pool, id, &nombre)
        .await
        .map_err(a_texto)
}

/// Solo elimina categorías sin productos; si tiene, devuelve un error para mostrar.
#[tauri::command]
pub async fn eliminar_categoria(estado: State<'_, EstadoApp>, id: i64) -> ResultadoComando<()> {
    srv::eliminar(&estado.pool, id).await.map_err(a_texto)
}
