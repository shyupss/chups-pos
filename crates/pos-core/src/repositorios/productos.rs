use sqlx::SqlitePool;

use crate::Resultado;

/// Cantidad de productos activos.
pub async fn contar_activos(pool: &SqlitePool) -> Resultado<i64> {
    let total = sqlx::query_scalar("SELECT COUNT(*) FROM productos WHERE activo = 1")
        .fetch_one(pool)
        .await?;
    Ok(total)
}
