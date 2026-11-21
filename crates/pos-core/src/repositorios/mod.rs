//! Acceso a datos. Solo SQL: sin reglas de negocio.
//!
//! Las funciones que escriben reciben una conexión (`&mut SqliteConnection`)
//! para poder ejecutarse dentro de la transacción que abre el servicio.

pub mod categorias;
pub mod clientes;
pub mod compras;
pub mod config;
pub mod devoluciones;
pub mod espera;
pub mod fiados;
pub mod inventario;
pub mod negocio;
pub mod productos;
pub mod proveedores;
pub mod reportes;
pub mod turnos;
pub mod ventas;
