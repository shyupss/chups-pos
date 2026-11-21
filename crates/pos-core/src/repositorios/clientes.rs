use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::repositorios::fiados::SALDO;
use crate::Resultado;

#[derive(Debug, Clone, FromRow)]
pub struct FilaCliente {
    pub id: i64,
    pub nombre: String,
    pub telefono: Option<String>,
    pub rut: Option<String>,
    pub direccion: Option<String>,
    pub nota: Option<String>,
    pub limite_credito: Option<i64>,
    pub activo: bool,
    pub creado_en: Option<String>,
    /// Suma de los saldos de sus ventas fiadas pendientes.
    pub deuda: i64,
}

fn columnas() -> String {
    format!(
        "c.id, c.nombre, c.telefono, c.rut, c.direccion, c.nota, c.limite_credito, c.activo,
         c.creado_en,
         COALESCE((SELECT SUM({SALDO}) FROM ventas v
                   WHERE v.cliente_id = c.id AND v.estado = 'fiada'), 0) AS deuda"
    )
}

pub struct Campos<'a> {
    pub nombre: &'a str,
    pub telefono: Option<&'a str>,
    pub rut: Option<&'a str>,
    pub direccion: Option<&'a str>,
    pub nota: Option<&'a str>,
    pub limite_credito: Option<i64>,
}

pub async fn por_id(conn: &mut SqliteConnection, id: i64) -> Resultado<Option<FilaCliente>> {
    let sql = format!("SELECT {} FROM clientes c WHERE c.id = ?", columnas());
    Ok(sqlx::query_as(&sql).bind(id).fetch_optional(conn).await?)
}

/// Clientes activos por nombre, teléfono o RUT (para elegir uno al fiar).
pub async fn buscar(pool: &SqlitePool, texto: &str, limite: i64) -> Resultado<Vec<FilaCliente>> {
    let sql = format!(
        "SELECT {} FROM clientes c
         WHERE c.activo = 1
           AND (c.nombre LIKE '%' || ?1 || '%' OR c.telefono LIKE '%' || ?1 || '%'
                OR replace(replace(c.rut, '.', ''), '-', '') LIKE '%' || ?2 || '%')
         ORDER BY c.nombre COLLATE NOCASE, c.id LIMIT ?3",
        columnas()
    );
    let sin_signos: String = texto.chars().filter(|c| c.is_alphanumeric()).collect();
    Ok(sqlx::query_as(&sql)
        .bind(texto)
        .bind(if sin_signos.is_empty() {
            "\u{0}".to_owned()
        } else {
            sin_signos
        })
        .bind(limite)
        .fetch_all(pool)
        .await?)
}

pub async fn insertar(conn: &mut SqliteConnection, c: &Campos<'_>) -> Resultado<i64> {
    Ok(sqlx::query_scalar(
        "INSERT INTO clientes (nombre, telefono, rut, direccion, nota, limite_credito, creado_en)
         VALUES (?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) RETURNING id",
    )
    .bind(c.nombre)
    .bind(c.telefono)
    .bind(c.rut)
    .bind(c.direccion)
    .bind(c.nota)
    .bind(c.limite_credito)
    .fetch_one(conn)
    .await?)
}

pub async fn actualizar(conn: &mut SqliteConnection, id: i64, c: &Campos<'_>) -> Resultado<u64> {
    Ok(sqlx::query(
        "UPDATE clientes SET nombre = ?, telefono = ?, rut = ?, direccion = ?, nota = ?,
                             limite_credito = ?
         WHERE id = ?",
    )
    .bind(c.nombre)
    .bind(c.telefono)
    .bind(c.rut)
    .bind(c.direccion)
    .bind(c.nota)
    .bind(c.limite_credito)
    .bind(id)
    .execute(conn)
    .await?
    .rows_affected())
}

pub async fn cambiar_activo(conn: &mut SqliteConnection, id: i64, activo: bool) -> Resultado<u64> {
    Ok(sqlx::query("UPDATE clientes SET activo = ? WHERE id = ?")
        .bind(activo)
        .bind(id)
        .execute(conn)
        .await?
        .rows_affected())
}
