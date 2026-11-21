use std::collections::HashMap;

use sqlx::SqliteConnection;

use crate::Resultado;

/// Toda la tabla config. Son pocas filas: una sola consulta.
pub async fn leer_todo(conn: &mut SqliteConnection) -> Resultado<HashMap<String, String>> {
    let filas: Vec<(String, String)> = sqlx::query_as("SELECT clave, valor FROM config")
        .fetch_all(conn)
        .await?;
    Ok(filas.into_iter().collect())
}

pub async fn guardar(conn: &mut SqliteConnection, clave: &str, valor: &str) -> Resultado<()> {
    sqlx::query(
        "INSERT INTO config (clave, valor) VALUES (?, ?)
         ON CONFLICT (clave) DO UPDATE SET valor = excluded.valor",
    )
    .bind(clave)
    .bind(valor)
    .execute(conn)
    .await?;
    Ok(())
}
