//! Inventario: stock, valorización, movimientos, ajustes por conteo y mermas.

use pos_core::servicios::inventario::{
    self as srv, Contado, CursorMovimientos, Diferencia, FiltrosMovimientos, FiltrosStock,
    MotivoMerma, PaginaMovimientos, ProductoStock, ResumenInventario,
};
use tauri::State;

use super::{a_texto, ResultadoComando, ResultadoProtegido};
use crate::estado::EstadoApp;

#[tauri::command]
pub async fn listar_stock(
    estado: State<'_, EstadoApp>,
    filtros: FiltrosStock,
    limite: i64,
    desplazamiento: i64,
) -> ResultadoComando<Vec<ProductoStock>> {
    srv::listar_stock(&estado.pool, &filtros, limite, desplazamiento)
        .await
        .map_err(a_texto)
}

#[tauri::command]
pub async fn resumen_inventario(
    estado: State<'_, EstadoApp>,
) -> ResultadoComando<ResumenInventario> {
    srv::resumen(&estado.pool).await.map_err(a_texto)
}

/// Productos bajo el mínimo, para la insignia de la barra lateral.
#[tauri::command]
pub async fn contar_bajo_minimo(estado: State<'_, EstadoApp>) -> ResultadoComando<i64> {
    srv::contar_bajo_minimo(&estado.pool).await.map_err(a_texto)
}

/// Una página del historial de movimientos; `despues_de` es el cursor de la página anterior.
#[tauri::command]
pub async fn listar_movimientos(
    estado: State<'_, EstadoApp>,
    filtros: FiltrosMovimientos,
    despues_de: Option<CursorMovimientos>,
    limite: i64,
) -> ResultadoComando<PaginaMovimientos> {
    srv::listar_movimientos(&estado.pool, &filtros, despues_de, limite)
        .await
        .map_err(a_texto)
}

/// Las diferencias que dejaría un conteo, sin guardar nada.
#[tauri::command]
pub async fn previsualizar_conteo(
    estado: State<'_, EstadoApp>,
    items: Vec<Contado>,
) -> ResultadoComando<Vec<Diferencia>> {
    srv::previsualizar_conteo(&estado.pool, &items)
        .await
        .map_err(a_texto)
}

/// Deja el stock en lo contado (una transacción). Pide el PIN si "ajustar inventario" está protegida.
#[tauri::command]
pub async fn ajustar_conteo(
    estado: State<'_, EstadoApp>,
    items: Vec<Contado>,
    motivo: String,
    pin: Option<String>,
) -> ResultadoProtegido<Vec<Diferencia>> {
    Ok(srv::ajustar_conteo(&estado.pool, &estado.pin, &items, &motivo, pin.as_deref()).await?)
}

#[tauri::command]
pub async fn registrar_merma(
    estado: State<'_, EstadoApp>,
    producto_id: i64,
    cantidad_mil: i64,
    motivo: MotivoMerma,
    detalle: Option<String>,
    pin: Option<String>,
) -> ResultadoProtegido<i64> {
    Ok(srv::registrar_merma(
        &estado.pool,
        &estado.pin,
        producto_id,
        cantidad_mil,
        motivo,
        detalle.as_deref(),
        pin.as_deref(),
    )
    .await?)
}
