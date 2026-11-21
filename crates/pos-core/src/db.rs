use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

use crate::Resultado;

/// Las migraciones de crates/pos-core/migrations, embebidas en el binario.
pub static MIGRACIONES: Migrator = sqlx::migrate!("./migrations");

/// Abre (o crea) la base de datos en `ruta` y la deja al día con las migraciones.
///
/// Configuración elegida por rendimiento en PCs modestos:
/// - WAL: las lecturas no bloquean a la escritura de una venta.
/// - `synchronous=NORMAL`: seguro con WAL y mucho más rápido que FULL.
/// - Pool chico: SQLite admite un solo escritor, más conexiones solo gastan memoria.
pub async fn abrir(ruta: &Path) -> Resultado<SqlitePool> {
    let opciones = SqliteConnectOptions::new()
        .filename(ruta)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5))
        // ~8 MB de caché de páginas por conexión.
        .pragma("cache_size", "-8000")
        .pragma("temp_store", "MEMORY");

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opciones)
        .await?;

    migrar(&pool).await?;
    Ok(pool)
}

/// Base de datos en memoria, para pruebas.
pub async fn abrir_en_memoria() -> Resultado<SqlitePool> {
    let opciones = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
    // Una sola conexión: cada conexión en memoria sería una base distinta.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opciones)
        .await?;
    migrar(&pool).await?;
    Ok(pool)
}

async fn migrar(pool: &SqlitePool) -> Resultado<()> {
    MIGRACIONES.run(pool).await?;
    Ok(())
}

/// Versión de esquema que trae esta versión de la app (la última migración embebida).
pub fn version_app() -> i64 {
    MIGRACIONES.iter().map(|m| m.version).max().unwrap_or(0)
}

/// Número de la última migración aplicada.
pub async fn version_esquema(pool: &SqlitePool) -> Resultado<i64> {
    let version: Option<i64> =
        sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1")
            .fetch_one(pool)
            .await?;
    Ok(version.unwrap_or(0))
}
