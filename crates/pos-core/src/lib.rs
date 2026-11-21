//! Modelo del POS (la "M" de MVC).
//!
//! Este crate no sabe nada de Tauri ni de la interfaz: los comandos de Tauri
//! (el controlador) lo llaman, y se puede probar con `cargo test` sin abrir ventanas.
//!
//! - [`db`]: abre SQLite con la configuración de rendimiento y aplica migraciones.
//! - [`repositorios`]: consultas SQL, una función por operación.
//! - [`servicios`]: reglas de negocio que combinan repositorios.

pub mod balanza;
pub mod boleta;
pub mod db;
pub mod error;
pub mod logo;
pub mod repositorios;
pub mod rut;
pub mod servicios;
pub mod zona;

pub use error::{Error, FalloPin, Resultado};
