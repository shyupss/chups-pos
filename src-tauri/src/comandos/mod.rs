//! Comandos Tauri: la única puerta de entrada desde la interfaz.
//!
//! Cada comando recibe, valida y delega en un servicio de `pos-core`.
//! Devuelve solo lo que la pantalla necesita mostrar.

pub mod actualizaciones;
pub mod asistente;
pub mod boletas;
pub mod categorias;
pub mod clientes;
pub mod compras;
pub mod config;
pub mod datos;
pub mod espera;
pub mod fiados;
pub mod impresion;
pub mod inventario;
pub mod licencia;
pub mod negocio;
pub mod productos;
pub mod reportes;
pub mod respaldos;
pub mod seguridad;
pub mod sistema;
pub mod turnos;
pub mod ventas;

use pos_core::FalloPin;
use serde::Serialize;

/// Los errores cruzan a la interfaz como texto legible.
pub type ResultadoComando<T> = Result<T, String>;

pub fn a_texto(e: pos_core::Error) -> String {
    e.to_string()
}

/// Error de un comando protegido con el PIN. Los demás errores siguen llegando como texto; los
/// del PIN llegan como `{ pin, mensaje, segundos }` para que la interfaz vuelva a pedirlo.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ErrorComando {
    Texto(String),
    #[serde(rename_all = "camelCase")]
    Pin {
        /// "requerido", "incorrecto" o "bloqueado".
        pin: &'static str,
        mensaje: String,
        /// Segundos que faltan para poder volver a probar, si está bloqueado.
        segundos: Option<u64>,
    },
}

pub type ResultadoProtegido<T> = Result<T, ErrorComando>;

/// Evento para la vista: cambió algo de lo que muestra la pantalla Hoy (ventas, caja, fiados o
/// stock). La pantalla se recarga al recibirlo, sin consultar cada tanto.
pub const EVENTO_CAMBIOS: &str = "cambios-del-dia";

pub(crate) fn avisar_cambios(app: &tauri::AppHandle) {
    use tauri::Emitter;
    // Si no hay ventanas escuchando no pasa nada: la pantalla se recarga al volver a ella.
    let _ = app.emit(EVENTO_CAMBIOS, ());
}

impl From<pos_core::Error> for ErrorComando {
    fn from(e: pos_core::Error) -> Self {
        match e {
            pos_core::Error::Pin(fallo) => {
                let (pin, segundos) = match fallo {
                    FalloPin::Requerido => ("requerido", None),
                    FalloPin::Incorrecto { .. } | FalloPin::CodigoIncorrecto { .. } => {
                        ("incorrecto", None)
                    }
                    FalloPin::Bloqueado { segundos } => ("bloqueado", Some(segundos)),
                };
                ErrorComando::Pin {
                    pin,
                    mensaje: fallo.to_string(),
                    segundos,
                }
            }
            otro => ErrorComando::Texto(otro.to_string()),
        }
    }
}

impl From<String> for ErrorComando {
    fn from(e: String) -> Self {
        ErrorComando::Texto(e)
    }
}
