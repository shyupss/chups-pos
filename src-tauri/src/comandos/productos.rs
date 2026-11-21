use pos_core::repositorios::productos::{FiltroCategoria, Producto};
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

/// Búsqueda paginada. `categoria_id` filtra por una categoría y `sin_categoria` muestra
/// solo los productos sin categoría; sin ninguno de los dos, busca en todas.
#[tauri::command]
pub async fn buscar_productos(
    estado: State<'_, EstadoApp>,
    texto: String,
    incluir_inactivos: bool,
    categoria_id: Option<i64>,
    sin_categoria: Option<bool>,
    limite: i64,
    desplazamiento: i64,
) -> ResultadoComando<Vec<Producto>> {
    let categoria = match (categoria_id, sin_categoria.unwrap_or(false)) {
        (_, true) => FiltroCategoria::SinCategoria,
        (Some(id), false) => FiltroCategoria::Una(id),
        (None, false) => FiltroCategoria::Todas,
    };
    srv::buscar(
        &estado.pool,
        &texto,
        incluir_inactivos,
        categoria,
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

/// Etiqueta de balanza (EAN-13 con prefijo 20–29): producto por kg y su peso.
/// `None` si no es una etiqueta de balanza o los códigos de balanza están desactivados.
#[tauri::command]
pub async fn leer_codigo_balanza(
    estado: State<'_, EstadoApp>,
    codigo: String,
) -> ResultadoComando<Option<pos_core::servicios::balanza::LecturaBalanza>> {
    pos_core::servicios::balanza::leer(&estado.pool, &codigo)
        .await
        .map_err(a_texto)
}
