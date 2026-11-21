use sqlx::{SqliteConnection, SqlitePool};

use crate::Resultado;

/// Valor de una clave de la tabla config, o `None` si no existe.
pub async fn leer(conn: &mut SqliteConnection, clave: &str) -> Resultado<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT valor FROM config WHERE clave = ?")
            .bind(clave)
            .fetch_optional(conn)
            .await?,
    )
}

pub async fn guardar(pool: &SqlitePool, clave: &str, valor: &str) -> Resultado<()> {
    sqlx::query(
        "INSERT INTO config (clave, valor) VALUES (?, ?)
         ON CONFLICT (clave) DO UPDATE SET valor = excluded.valor",
    )
    .bind(clave)
    .bind(valor)
    .execute(pool)
    .await?;
    Ok(())
}
