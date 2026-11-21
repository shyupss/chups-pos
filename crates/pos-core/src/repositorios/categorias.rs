use serde::Serialize;
use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::Resultado;

#[derive(Debug, Clone, Serialize, FromRow, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Categoria {
    pub id: i64,
    pub nombre: String,
    /// Productos activos de la categoría.
    pub productos: i64,
}

const SELECT: &str = "SELECT c.id, c.nombre,
        (SELECT COUNT(*) FROM productos p WHERE p.categoria_id = c.id AND p.activo = 1) AS productos
     FROM categorias c";

/// Todas las categorías, por nombre. Son pocas: no se paginan.
pub async fn listar(pool: &SqlitePool) -> Resultado<Vec<Categoria>> {
    let sql = format!("{SELECT} ORDER BY c.nombre COLLATE NOCASE, c.id");
    Ok(sqlx::query_as(&sql).fetch_all(pool).await?)
}

pub async fn por_id(pool: &SqlitePool, id: i64) -> Resultado<Option<Categoria>> {
    let sql = format!("{SELECT} WHERE c.id = ?");
    Ok(sqlx::query_as(&sql).bind(id).fetch_optional(pool).await?)
}

pub async fn existe(conn: &mut SqliteConnection, id: i64) -> Resultado<bool> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM categorias WHERE id = ?")
        .bind(id)
        .fetch_one(conn)
        .await?;
    Ok(n > 0)
}

pub async fn insertar(pool: &SqlitePool, nombre: &str) -> Resultado<i64> {
    let id = sqlx::query_scalar("INSERT INTO categorias (nombre) VALUES (?) RETURNING id")
        .bind(nombre)
        .fetch_one(pool)
        .await?;
    Ok(id)
}

pub async fn renombrar(pool: &SqlitePool, id: i64, nombre: &str) -> Resultado<u64> {
    let r = sqlx::query("UPDATE categorias SET nombre = ? WHERE id = ?")
        .bind(nombre)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

/// Productos que apuntan a la categoría: (todos, desactivados).
pub async fn contar_productos(conn: &mut SqliteConnection, id: i64) -> Resultado<(i64, i64)> {
    Ok(sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(activo = 0), 0) FROM productos WHERE categoria_id = ?",
    )
    .bind(id)
    .fetch_one(conn)
    .await?)
}

pub async fn eliminar(conn: &mut SqliteConnection, id: i64) -> Resultado<u64> {
    let r = sqlx::query("DELETE FROM categorias WHERE id = ?")
        .bind(id)
        .execute(conn)
        .await?;
    Ok(r.rows_affected())
}
