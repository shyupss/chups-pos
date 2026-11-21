//! Comandos Tauri: la única puerta de entrada desde la interfaz.
//!
//! Cada comando recibe, valida y delega en un servicio de `pos-core`.
//! Devuelve solo lo que la pantalla necesita mostrar.

pub mod categorias;
pub mod config;
pub mod espera;
pub mod negocio;
pub mod productos;
pub mod sistema;
pub mod ventas;

/// Los errores cruzan a la interfaz como texto legible.
pub type ResultadoComando<T> = Result<T, String>;

pub fn a_texto(e: pos_core::Error) -> String {
    e.to_string()
}
