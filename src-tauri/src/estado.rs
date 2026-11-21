use std::path::PathBuf;

use sqlx::SqlitePool;

/// Estado compartido por todos los comandos. Se crea una vez al arrancar.
pub struct EstadoApp {
    pub pool: SqlitePool,
    pub ruta_db: PathBuf,
    /// Milisegundos desde que arrancó el proceso hasta que la base quedó lista.
    pub ms_inicio: u64,
}
