//! Acceso a datos. Solo SQL: sin reglas de negocio.
//!
//! Las funciones que escriben reciben una conexión (`&mut SqliteConnection`)
//! para poder ejecutarse dentro de la transacción que abre el servicio.

pub mod productos;
pub mod ventas;
