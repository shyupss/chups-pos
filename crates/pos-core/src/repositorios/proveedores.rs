use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::Resultado;

#[derive(Debug, Clone, FromRow)]
pub struct FilaProveedor {
    pub id: i64,
    pub nombre: String,
    pub rut: Option<String>,
    pub telefono: Option<String>,
    pub contacto: Option<String>,
    pub nota: Option<String>,
    pub activo: bool,
    pub compras: i64,
    pub ultima_compra: Option<String>,
}

const COLUMNAS: &str = "p.id, p.nombre, p.rut, p.telefono, p.contacto, p.nota, p.activo,
    (SELECT COUNT(*) FROM compras c WHERE c.proveedor_id = p.id AND c.anulada_en IS NULL) AS compras,
    (SELECT MAX(c.fecha) FROM compras c WHERE c.proveedor_id = p.id AND c.anulada_en IS NULL)
        AS ultima_compra";

pub struct Campos<'a> {
    pub nombre: &'a str,
    pub rut: Option<&'a str>,
    pub telefono: Option<&'a str>,
    pub contacto: Option<&'a str>,
    pub nota: Option<&'a str>,
}

/// Por nombre, contacto o RUT; ordenados por nombre.
pub async fn listar(
    pool: &SqlitePool,
    texto: Option<&str>,
    incluir_inactivos: bool,
    limite: i64,
    desplazamiento: i64,
) -> Resultado<Vec<FilaProveedor>> {
    let sql = format!(
        "SELECT {COLUMNAS} FROM proveedores p
         WHERE (?1 OR p.activo = 1)
           AND (?2 IS NULL OR p.nombre LIKE '%' || ?2 || '%' OR p.contacto LIKE '%' || ?2 || '%'
                OR p.rut LIKE '%' || ?2 || '%')
         ORDER BY p.nombre COLLATE NOCASE, p.id LIMIT ?3 OFFSET ?4"
    );
    Ok(sqlx::query_as(&sql)
        .bind(incluir_inactivos)
        .bind(texto)
        .bind(limite)
        .bind(desplazamiento)
        .fetch_all(pool)
        .await?)
}

pub async fn por_id(conn: &mut SqliteConnection, id: i64) -> Resultado<Option<FilaProveedor>> {
    let sql = format!("SELECT {COLUMNAS} FROM proveedores p WHERE p.id = ?");
    Ok(sqlx::query_as(&sql).bind(id).fetch_optional(conn).await?)
}

pub async fn insertar(conn: &mut SqliteConnection, c: &Campos<'_>) -> Resultado<i64> {
    Ok(sqlx::query_scalar(
        "INSERT INTO proveedores (nombre, rut, telefono, contacto, nota) VALUES (?, ?, ?, ?, ?)
         RETURNING id",
    )
    .bind(c.nombre)
    .bind(c.rut)
    .bind(c.telefono)
    .bind(c.contacto)
    .bind(c.nota)
    .fetch_one(conn)
    .await?)
}

pub async fn actualizar(conn: &mut SqliteConnection, id: i64, c: &Campos<'_>) -> Resultado<u64> {
    Ok(sqlx::query(
        "UPDATE proveedores SET nombre = ?, rut = ?, telefono = ?, contacto = ?, nota = ?
         WHERE id = ?",
    )
    .bind(c.nombre)
    .bind(c.rut)
    .bind(c.telefono)
    .bind(c.contacto)
    .bind(c.nota)
    .bind(id)
    .execute(conn)
    .await?
    .rows_affected())
}

pub async fn cambiar_activo(pool: &SqlitePool, id: i64, activo: bool) -> Resultado<u64> {
    Ok(
        sqlx::query("UPDATE proveedores SET activo = ? WHERE id = ?")
            .bind(activo)
            .bind(id)
            .execute(pool)
            .await?
            .rows_affected(),
    )
}
