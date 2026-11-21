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
