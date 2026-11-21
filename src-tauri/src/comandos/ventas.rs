use pos_core::servicios::historial::{self, DetalleVenta, FiltrosVentas, PaginaVentas};
use pos_core::servicios::ventas::{self as srv, NuevaVenta, VentaConfirmada};
use tauri::State;

use super::{a_texto, ResultadoComando};
use crate::estado::EstadoApp;

#[tauri::command]
pub async fn confirmar_venta(
    estado: State<'_, EstadoApp>,
    venta: NuevaVenta,
) -> ResultadoComando<VentaConfirmada> {
    srv::confirmar(&estado.pool, &venta).await.map_err(a_texto)
}

/// Una página del historial con los filtros de la pantalla Ventas (y los totales en la primera).
#[tauri::command]
pub async fn listar_ventas(
    estado: State<'_, EstadoApp>,
    filtros: FiltrosVentas,
    limite: i64,
    desplazamiento: i64,
) -> ResultadoComando<PaginaVentas> {
    historial::listar(&estado.pool, &filtros, limite, desplazamiento)
        .await
        .map_err(a_texto)
}

#[tauri::command]
pub async fn detalle_venta(
    estado: State<'_, EstadoApp>,
    id: i64,
) -> ResultadoComando<DetalleVenta> {
    historial::detalle(&estado.pool, id).await.map_err(a_texto)
}

/// Id de la venta con ese folio (escrito o escaneado del código de la boleta).
#[tauri::command]
pub async fn venta_por_folio(
    estado: State<'_, EstadoApp>,
    folio: i64,
) -> ResultadoComando<Option<i64>> {
    historial::por_folio(&estado.pool, folio)
        .await
        .map_err(a_texto)
}
