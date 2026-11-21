use serde::Serialize;
use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::Resultado;

#[derive(Debug, Clone, Serialize, FromRow, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Producto {
    pub id: i64,
    pub codigo_barras: Option<String>,
    pub nombre: String,
    pub precio_venta: i64,
    pub costo: Option<i64>,
    pub stock: i64,
    pub stock_minimo: i64,
    pub activo: bool,
}

const COLUMNAS: &str =
    "p.id, p.codigo_barras, p.nombre, p.precio_venta, p.costo, p.stock, p.stock_minimo, p.activo";

/// Campos que se guardan al crear o editar.
pub struct Campos<'a> {
    pub codigo_barras: Option<&'a str>,
    pub nombre: &'a str,
    pub precio_venta: i64,
    pub costo: Option<i64>,
    pub stock_minimo: i64,
}

/// Cantidad de productos activos.
pub async fn contar_activos(pool: &SqlitePool) -> Resultado<i64> {
    let total = sqlx::query_scalar("SELECT COUNT(*) FROM productos WHERE activo = 1")
        .fetch_one(pool)
        .await?;
    Ok(total)
}

/// Producto activo por código exacto. Es la consulta del escaneo: usa el índice único.
pub async fn por_codigo(pool: &SqlitePool, codigo: &str) -> Resultado<Option<Producto>> {
    let sql =
        format!("SELECT {COLUMNAS} FROM productos p WHERE p.codigo_barras = ? AND p.activo = 1");
    Ok(sqlx::query_as(&sql)
        .bind(codigo)
        .fetch_optional(pool)
        .await?)
}

pub async fn por_id(conn: &mut SqliteConnection, id: i64) -> Resultado<Option<Producto>> {
    let sql = format!("SELECT {COLUMNAS} FROM productos p WHERE p.id = ?");
    Ok(sqlx::query_as(&sql).bind(id).fetch_optional(conn).await?)
}

/// Lista paginada ordenada por nombre. `consulta_fts` filtra por nombre (FTS5);
/// `prefijo_codigo` además acepta productos cuyo código empiece así.
pub async fn listar(
    pool: &SqlitePool,
    consulta_fts: Option<&str>,
    prefijo_codigo: Option<&str>,
    incluir_inactivos: bool,
    limite: i64,
    desplazamiento: i64,
) -> Resultado<Vec<Producto>> {
    let mut sql = format!("SELECT {COLUMNAS} FROM productos p WHERE 1 = 1");
    if !incluir_inactivos {
        sql.push_str(" AND p.activo = 1");
    }
    match (consulta_fts.is_some(), prefijo_codigo.is_some()) {
        (true, true) => sql.push_str(
            " AND (p.id IN (SELECT rowid FROM productos_fts WHERE productos_fts MATCH ?1)
                   OR p.codigo_barras LIKE ?2 || '%')",
        ),
        (true, false) => sql.push_str(
            " AND p.id IN (SELECT rowid FROM productos_fts WHERE productos_fts MATCH ?1)",
        ),
        (false, true) => sql.push_str(" AND p.codigo_barras LIKE ?2 || '%'"),
        (false, false) => {}
    }
    sql.push_str(" ORDER BY p.nombre COLLATE NOCASE, p.id LIMIT ?3 OFFSET ?4");

    Ok(sqlx::query_as(&sql)
        .bind(consulta_fts.unwrap_or_default())
        .bind(prefijo_codigo.unwrap_or_default())
        .bind(limite)
        .bind(desplazamiento)
        .fetch_all(pool)
        .await?)
}

pub async fn insertar(conn: &mut SqliteConnection, c: &Campos<'_>) -> Resultado<i64> {
    let id = sqlx::query_scalar(
        "INSERT INTO productos (codigo_barras, nombre, precio_venta, costo, stock_minimo)
         VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(c.codigo_barras)
    .bind(c.nombre)
    .bind(c.precio_venta)
    .bind(c.costo)
    .bind(c.stock_minimo)
    .fetch_one(conn)
    .await?;
    Ok(id)
}

pub async fn actualizar(conn: &mut SqliteConnection, id: i64, c: &Campos<'_>) -> Resultado<()> {
    sqlx::query(
        "UPDATE productos
         SET codigo_barras = ?, nombre = ?, precio_venta = ?, costo = ?, stock_minimo = ?
         WHERE id = ?",
    )
    .bind(c.codigo_barras)
    .bind(c.nombre)
    .bind(c.precio_venta)
    .bind(c.costo)
    .bind(c.stock_minimo)
    .bind(id)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn cambiar_activo(pool: &SqlitePool, id: i64, activo: bool) -> Resultado<u64> {
    let r = sqlx::query("UPDATE productos SET activo = ? WHERE id = ?")
        .bind(activo)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn registrar_cambio_precio(
    conn: &mut SqliteConnection,
    producto_id: i64,
    anterior: i64,
    nuevo: i64,
) -> Resultado<()> {
    sqlx::query(
        "INSERT INTO historial_precios (producto_id, precio_anterior, precio_nuevo) VALUES (?, ?, ?)",
    )
    .bind(producto_id)
    .bind(anterior)
    .bind(nuevo)
    .execute(conn)
    .await?;
    Ok(())
}

/// Suma `delta` al stock (negativo para descontar) y deja el movimiento en el historial.
/// Devuelve el stock resultante.
pub async fn mover_stock(
    conn: &mut SqliteConnection,
    producto_id: i64,
    delta: i64,
    tipo: &str,
    venta_id: Option<i64>,
    nota: Option<&str>,
) -> Resultado<i64> {
    let stock: i64 =
        sqlx::query_scalar("UPDATE productos SET stock = stock + ? WHERE id = ? RETURNING stock")
            .bind(delta)
            .bind(producto_id)
            .fetch_one(&mut *conn)
            .await?;
    sqlx::query(
        "INSERT INTO movimientos_stock (producto_id, tipo, cantidad, stock_resultante, venta_id, nota)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(producto_id)
    .bind(tipo)
    .bind(delta)
    .bind(stock)
    .bind(venta_id)
    .bind(nota)
    .execute(conn)
    .await?;
    Ok(stock)
}
