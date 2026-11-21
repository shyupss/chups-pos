//! Fiados: deudores, ficha del cliente, pagos y sus comprobantes.

use pos_core::servicios::fiados::{
    self as srv, FichaCliente, NuevoPagoCliente, PaginaDeudores, PagoRegistrado,
};
use pos_core::servicios::impresion;
use tauri::{AppHandle, State};

use super::{a_texto, ResultadoComando, ResultadoProtegido};
use crate::estado::EstadoApp;

/// Clientes con deuda (o todos), de la deuda mayor a la menor.
#[tauri::command]
pub async fn listar_deudores(
    estado: State<'_, EstadoApp>,
    texto: String,
    todos: bool,
    limite: i64,
    desplazamiento: i64,
) -> ResultadoComando<PaginaDeudores> {
    srv::listar_deudores(&estado.pool, &texto, todos, limite, desplazamiento)
        .await
        .map_err(a_texto)
}

/// El cliente con sus ventas pendientes, las ya pagadas y sus pagos.
#[tauri::command]
pub async fn ficha_cliente(
    estado: State<'_, EstadoApp>,
    id: i64,
) -> ResultadoComando<FichaCliente> {
    srv::ficha(&estado.pool, id).await.map_err(a_texto)
}

#[tauri::command]
pub async fn registrar_pago(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
    pago: NuevoPagoCliente,
) -> ResultadoComando<PagoRegistrado> {
    let r = srv::registrar_pago(&estado.pool, &pago)
        .await
        .map_err(a_texto)?;
    super::avisar_cambios(&app);
    Ok(r)
}

/// Anula un pago mal ingresado (PIN de anular venta).
#[tauri::command]
pub async fn anular_pago(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
    id: i64,
    motivo: String,
    pin: Option<String>,
) -> ResultadoProtegido<PagoRegistrado> {
    let r = srv::anular_pago(&estado.pool, &estado.pin, id, &motivo, pin.as_deref()).await?;
    super::avisar_cambios(&app);
    Ok(r)
}

/// Imprime el "COMPROBANTE DE PAGO" en la impresora configurada.
#[tauri::command]
pub async fn imprimir_pago(estado: State<'_, EstadoApp>, pago_id: i64) -> ResultadoComando<()> {
    let trabajo = impresion::trabajo_pago(&estado.pool, pago_id)
        .await
        .map_err(a_texto)?;
    super::impresion::enviar(&estado, trabajo).await
}

/// Imprime el "ESTADO DE CUENTA" del cliente.
#[tauri::command]
pub async fn imprimir_estado_cuenta(
    estado: State<'_, EstadoApp>,
    cliente_id: i64,
) -> ResultadoComando<()> {
    let trabajo = impresion::trabajo_estado_cuenta(&estado.pool, cliente_id)
        .await
        .map_err(a_texto)?;
    super::impresion::enviar(&estado, trabajo).await
}

/// Guarda el PDF del estado de cuenta junto a los de las boletas.
#[tauri::command]
pub async fn guardar_estado_cuenta_pdf(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
    cliente_id: i64,
    abrir: bool,
) -> ResultadoComando<String> {
    let pendiente = impresion::preparar_pdf_estado_cuenta(&estado.pool, cliente_id)
        .await
        .map_err(a_texto)?;
    super::boletas::guardar_pdf(&app, &estado, pendiente, abrir).await
}
