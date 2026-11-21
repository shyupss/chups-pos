use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::Resultado;

/// Cómo se vende un producto. En los productos por kg, `precio_venta` es el precio por kilo.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum Unidad {
    #[default]
    Unidad,
    Kg,
}

/// Las cantidades van en milésimas: 1 unidad = 1000, 0,532 kg = 532.
#[derive(Debug, Clone, Serialize, FromRow, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Producto {
    pub id: i64,
    pub codigo_barras: Option<String>,
    pub nombre: String,
    pub categoria_id: Option<i64>,
    pub unidad: Unidad,
    pub precio_venta: i64,
    pub costo: Option<i64>,
    pub stock_mil: i64,
    pub stock_minimo_mil: i64,
    pub activo: bool,
}

const COLUMNAS: &str = "p.id, p.codigo_barras, p.nombre, p.categoria_id, p.unidad, \
                        p.precio_venta, p.costo, p.stock_mil, p.stock_minimo_mil, p.activo";

/// Campos que se guardan al crear o editar.
pub struct Campos<'a> {
    pub codigo_barras: Option<&'a str>,
    pub nombre: &'a str,
    pub categoria_id: Option<i64>,
    pub unidad: Unidad,
    pub precio_venta: i64,
    pub costo: Option<i64>,
    pub stock_minimo_mil: i64,
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

/// Filtro de categoría de la búsqueda.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FiltroCategoria {
    #[default]
    Todas,
    SinCategoria,
    Una(i64),
}

/// Lista paginada ordenada por nombre. `consulta_fts` filtra por nombre (FTS5);
/// `prefijo_codigo` además acepta productos cuyo código empiece así.
pub async fn listar(
    pool: &SqlitePool,
    consulta_fts: Option<&str>,
    prefijo_codigo: Option<&str>,
    incluir_inactivos: bool,
    categoria: FiltroCategoria,
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
    match categoria {
        FiltroCategoria::Todas => {}
        FiltroCategoria::SinCategoria => sql.push_str(" AND p.categoria_id IS NULL"),
        FiltroCategoria::Una(_) => sql.push_str(" AND p.categoria_id = ?3"),
    }
    // LIMIT y OFFSET van al final (?4 y ?5) para que los cinco parámetros siempre existan.
    sql.push_str(" ORDER BY p.nombre COLLATE NOCASE, p.id LIMIT ?4 OFFSET ?5");
    let categoria_id = match categoria {
        FiltroCategoria::Una(id) => id,
        _ => 0,
    };

    Ok(sqlx::query_as(&sql)
        .bind(consulta_fts.unwrap_or_default())
        .bind(prefijo_codigo.unwrap_or_default())
        .bind(categoria_id)
        .bind(limite)
        .bind(desplazamiento)
        .fetch_all(pool)
        .await?)
}

pub async fn insertar(conn: &mut SqliteConnection, c: &Campos<'_>) -> Resultado<i64> {
    let id = sqlx::query_scalar(
        "INSERT INTO productos
            (codigo_barras, nombre, categoria_id, unidad, precio_venta, costo, stock_minimo_mil)
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(c.codigo_barras)
    .bind(c.nombre)
    .bind(c.categoria_id)
    .bind(c.unidad)
    .bind(c.precio_venta)
    .bind(c.costo)
    .bind(c.stock_minimo_mil)
    .fetch_one(conn)
    .await?;
    Ok(id)
}

pub async fn actualizar(conn: &mut SqliteConnection, id: i64, c: &Campos<'_>) -> Resultado<()> {
    sqlx::query(
        "UPDATE productos
         SET codigo_barras = ?, nombre = ?, categoria_id = ?, unidad = ?, precio_venta = ?,
             costo = ?, stock_minimo_mil = ?
         WHERE id = ?",
    )
    .bind(c.codigo_barras)
    .bind(c.nombre)
    .bind(c.categoria_id)
    .bind(c.unidad)
    .bind(c.precio_venta)
    .bind(c.costo)
    .bind(c.stock_minimo_mil)
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

/// Suma `delta_mil` al stock (negativo para descontar) y deja el movimiento en el historial.
/// Devuelve el stock resultante, en milésimas.
pub async fn mover_stock(
    conn: &mut SqliteConnection,
    producto_id: i64,
    delta_mil: i64,
    tipo: &str,
    venta_id: Option<i64>,
    nota: Option<&str>,
) -> Resultado<i64> {
    registrar_movimiento(
        conn,
        &Movimiento {
            producto_id,
            delta_mil,
            tipo,
            venta_id,
            compra_id: None,
            nota,
            motivo: None,
        },
    )
    .await
}

/// Un cambio de stock con todo su origen: venta, compra, nota y motivo (ajustes y mermas).
pub struct Movimiento<'a> {
    pub producto_id: i64,
    /// Positivo entra, negativo sale; cero deja la merma anotada sin mover el stock.
    pub delta_mil: i64,
    pub tipo: &'a str,
    pub venta_id: Option<i64>,
    pub compra_id: Option<i64>,
    pub nota: Option<&'a str>,
    pub motivo: Option<&'a str>,
}

/// Aplica el movimiento al stock y lo deja en el historial. Devuelve el stock resultante.
pub async fn registrar_movimiento(
    conn: &mut SqliteConnection,
    m: &Movimiento<'_>,
) -> Resultado<i64> {
    let stock_mil: i64 = sqlx::query_scalar(
        "UPDATE productos SET stock_mil = stock_mil + ? WHERE id = ? RETURNING stock_mil",
    )
    .bind(m.delta_mil)
    .bind(m.producto_id)
    .fetch_one(&mut *conn)
    .await?;
    sqlx::query(
        "INSERT INTO movimientos_stock
            (producto_id, tipo, cantidad_mil, stock_resultante_mil, venta_id, compra_id, nota,
             motivo)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(m.producto_id)
    .bind(m.tipo)
    .bind(m.delta_mil)
    .bind(stock_mil)
    .bind(m.venta_id)
    .bind(m.compra_id)
    .bind(m.nota)
    .bind(m.motivo)
    .execute(conn)
    .await?;
    Ok(stock_mil)
}

/// El último costo unitario (por unidad o por kg), al registrar una compra.
pub async fn actualizar_costo(
    conn: &mut SqliteConnection,
    producto_id: i64,
    costo: i64,
) -> Resultado<()> {
    sqlx::query("UPDATE productos SET costo = ? WHERE id = ?")
        .bind(costo)
        .bind(producto_id)
        .execute(conn)
        .await?;
    Ok(())
}
