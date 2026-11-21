use pos_core::repositorios::espera::ResumenEspera;
use pos_core::servicios::espera::{self as srv, NuevaEnEspera, VentaRecuperada};
use pos_core::servicios::ventas::ItemPedido;
use tauri::State;

use super::{a_texto, ResultadoComando};
use crate::estado::EstadoApp;

/// Deja el carrito en espera (nombre opcional) y devuelve su id.
#[tauri::command]
pub async fn poner_en_espera(
    estado: State<'_, EstadoApp>,
    nombre: Option<String>,
    items: Vec<ItemPedido>,
) -> ResultadoComando<i64> {
    srv::poner(&estado.pool, &NuevaEnEspera { nombre, items })
        .await
        .map_err(a_texto)
}

#[tauri::command]
pub async fn listar_en_espera(
    estado: State<'_, EstadoApp>,
) -> ResultadoComando<Vec<ResumenEspera>> {
    srv::listar(&estado.pool).await.map_err(a_texto)
}

/// Devuelve las líneas y borra la venta en espera. Con `intercambio`, el carrito actual
/// queda en espera en la misma transacción.
#[tauri::command]
pub async fn recuperar_en_espera(
    estado: State<'_, EstadoApp>,
    id: i64,
    intercambio: Option<NuevaEnEspera>,
) -> ResultadoComando<VentaRecuperada> {
    srv::recuperar(&estado.pool, id, intercambio.as_ref())
        .await
        .map_err(a_texto)
}

#[tauri::command]
pub async fn descartar_en_espera(estado: State<'_, EstadoApp>, id: i64) -> ResultadoComando<()> {
    srv::descartar(&estado.pool, id).await.map_err(a_texto)
}
