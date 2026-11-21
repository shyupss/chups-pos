use pos_core::servicios::devoluciones::{self, DevolucionRegistrada, ItemDevuelto};
use pos_core::servicios::historial::{self, DetalleVenta, FiltrosVentas, PaginaVentas};
use pos_core::servicios::ventas::{self as srv, MedioPago, NuevaVenta, VentaConfirmada};
use pos_core::servicios::{anulacion, impresion};
use tauri::State;

use super::{a_texto, ResultadoComando, ResultadoProtegido};
use crate::estado::EstadoApp;

/// Confirma la venta. Una fiada sobre el límite de crédito se rechaza, salvo con
/// `sobre_limite`, que pide el PIN si "fiar sobre el límite" está protegida.
#[tauri::command]
pub async fn confirmar_venta(
    estado: State<'_, EstadoApp>,
    venta: NuevaVenta,
    sobre_limite: Option<bool>,
    pin: Option<String>,
) -> ResultadoProtegido<VentaConfirmada> {
    Ok(srv::confirmar_autorizada(
        &estado.pool,
        &estado.pin,
        &venta,
        sobre_limite.unwrap_or(false),
        pin.as_deref(),
    )
    .await?)
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

/// Anula la venta (repone el stock y registra el reembolso). Pide el PIN si está protegido.
#[tauri::command]
pub async fn anular_venta(
    estado: State<'_, EstadoApp>,
    id: i64,
    motivo: String,
    medio_reembolso: Option<MedioPago>,
    pin: Option<String>,
) -> ResultadoProtegido<DetalleVenta> {
    Ok(anulacion::anular(
        &estado.pool,
        &estado.pin,
        id,
        &motivo,
        medio_reembolso,
        pin.as_deref(),
    )
    .await?)
}

/// Imprime el comprobante "ANULACIÓN DE VENTA" en la impresora configurada.
#[tauri::command]
pub async fn imprimir_anulacion(
    estado: State<'_, EstadoApp>,
    venta_id: i64,
) -> ResultadoComando<()> {
    let trabajo = impresion::trabajo_anulacion(&estado.pool, venta_id)
        .await
        .map_err(a_texto)?;
    super::impresion::enviar(&estado, trabajo).await
}

/// Devuelve parte de una venta. Pide el PIN si "devolución" está protegida.
#[tauri::command]
pub async fn registrar_devolucion(
    estado: State<'_, EstadoApp>,
    venta_id: i64,
    items: Vec<ItemDevuelto>,
    medio_reembolso: Option<MedioPago>,
    motivo: String,
    pin: Option<String>,
) -> ResultadoProtegido<DevolucionRegistrada> {
    Ok(devoluciones::registrar(
        &estado.pool,
        &estado.pin,
        venta_id,
        &items,
        medio_reembolso,
        &motivo,
        pin.as_deref(),
    )
    .await?)
}

/// Imprime la "NOTA DE DEVOLUCIÓN" en la impresora configurada.
#[tauri::command]
pub async fn imprimir_devolucion(
    estado: State<'_, EstadoApp>,
    devolucion_id: i64,
) -> ResultadoComando<()> {
    let trabajo = impresion::trabajo_devolucion(&estado.pool, devolucion_id)
        .await
        .map_err(a_texto)?;
    super::impresion::enviar(&estado, trabajo).await
}
