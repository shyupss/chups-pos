//! Compras a proveedores y proveedores.

use pos_core::servicios::compras::{
    self as srv, Compra, DetalleCompra, FiltrosCompras, NuevaCompra,
};
use pos_core::servicios::proveedores::{self, DatosProveedor, Proveedor};
use tauri::{AppHandle, State};

use super::{a_texto, ResultadoComando, ResultadoProtegido};
use crate::estado::EstadoApp;

#[tauri::command]
pub async fn listar_proveedores(
    estado: State<'_, EstadoApp>,
    texto: String,
    incluir_inactivos: bool,
    limite: i64,
    desplazamiento: i64,
) -> ResultadoComando<Vec<Proveedor>> {
    proveedores::listar(
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
pub async fn crear_proveedor(
    estado: State<'_, EstadoApp>,
    datos: DatosProveedor,
) -> ResultadoComando<Proveedor> {
    proveedores::crear(&estado.pool, &datos)
        .await
        .map_err(a_texto)
}

#[tauri::command]
pub async fn editar_proveedor(
    estado: State<'_, EstadoApp>,
    id: i64,
    datos: DatosProveedor,
) -> ResultadoComando<Proveedor> {
    proveedores::editar(&estado.pool, id, &datos)
        .await
        .map_err(a_texto)
}

#[tauri::command]
pub async fn cambiar_estado_proveedor(
    estado: State<'_, EstadoApp>,
    id: i64,
    activo: bool,
) -> ResultadoComando<Proveedor> {
    proveedores::cambiar_activo(&estado.pool, id, activo)
        .await
        .map_err(a_texto)
}

/// Registra la compra. Si se paga con la caja, pide el PIN de los retiros cuando está protegido.
#[tauri::command]
pub async fn registrar_compra(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
    compra: NuevaCompra,
    pin: Option<String>,
) -> ResultadoProtegido<DetalleCompra> {
    let r = srv::registrar(&estado.pool, &estado.pin, &compra, pin.as_deref()).await?;
    super::avisar_cambios(&app);
    Ok(r)
}

#[tauri::command]
pub async fn detalle_compra(
    estado: State<'_, EstadoApp>,
    id: i64,
) -> ResultadoComando<DetalleCompra> {
    srv::detalle(&estado.pool, id).await.map_err(a_texto)
}

#[tauri::command]
pub async fn listar_compras(
    estado: State<'_, EstadoApp>,
    filtros: FiltrosCompras,
    limite: i64,
    desplazamiento: i64,
) -> ResultadoComando<Vec<Compra>> {
    srv::listar(&estado.pool, &filtros, limite, desplazamiento)
        .await
        .map_err(a_texto)
}

/// Anula la compra (compensa stock y caja). Pide el PIN de "ajustar inventario".
#[tauri::command]
pub async fn anular_compra(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
    id: i64,
    motivo: String,
    pin: Option<String>,
) -> ResultadoProtegido<DetalleCompra> {
    let r = srv::anular(&estado.pool, &estado.pin, id, &motivo, pin.as_deref()).await?;
    super::avisar_cambios(&app);
    Ok(r)
}
