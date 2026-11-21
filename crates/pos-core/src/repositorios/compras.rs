use sqlx::{FromRow, QueryBuilder, Sqlite, SqliteConnection, SqlitePool};

use crate::repositorios::productos::Unidad;
use crate::Resultado;

pub struct NuevaCompra<'a> {
    pub proveedor_id: Option<i64>,
    pub documento: Option<&'a str>,
    pub total: i64,
    pub nota: Option<&'a str>,
    pub pagada_con_caja: bool,
}

pub async fn insertar(conn: &mut SqliteConnection, c: &NuevaCompra<'_>) -> Resultado<i64> {
    Ok(sqlx::query_scalar(
        "INSERT INTO compras (proveedor_id, documento, total, nota, pagada_con_caja)
         VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(c.proveedor_id)
    .bind(c.documento)
    .bind(c.total)
    .bind(c.nota)
    .bind(c.pagada_con_caja)
    .fetch_one(conn)
    .await?)
}

pub async fn insertar_item(
    conn: &mut SqliteConnection,
    compra_id: i64,
    producto_id: i64,
    cantidad_mil: i64,
    costo_unitario: i64,
    subtotal: i64,
) -> Resultado<()> {
    sqlx::query(
        "INSERT INTO compra_items (compra_id, producto_id, cantidad_mil, costo_unitario, subtotal)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(compra_id)
    .bind(producto_id)
    .bind(cantidad_mil)
    .bind(costo_unitario)
    .bind(subtotal)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn fijar_movimiento_caja(
    conn: &mut SqliteConnection,
    compra_id: i64,
    movimiento_id: i64,
) -> Resultado<()> {
    sqlx::query("UPDATE compras SET movimiento_caja_id = ? WHERE id = ?")
        .bind(movimiento_id)
        .bind(compra_id)
        .execute(conn)
        .await?;
    Ok(())
}

/// Marca la compra como anulada si no lo estaba. `false` si ya lo estaba.
pub async fn anular(conn: &mut SqliteConnection, id: i64, motivo: &str) -> Resultado<bool> {
    let r = sqlx::query(
        "UPDATE compras SET anulada_en = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), motivo_anulacion = ?
         WHERE id = ? AND anulada_en IS NULL",
    )
    .bind(motivo)
    .bind(id)
    .execute(conn)
    .await?;
    Ok(r.rows_affected() == 1)
}

#[derive(Debug, Clone, FromRow)]
pub struct FilaCompra {
    pub id: i64,
    pub fecha: String,
    pub proveedor_id: Option<i64>,
    pub proveedor: Option<String>,
    pub documento: Option<String>,
    pub total: i64,
    pub nota: Option<String>,
    pub pagada_con_caja: bool,
    pub movimiento_caja_id: Option<i64>,
    pub anulada_en: Option<String>,
    pub motivo_anulacion: Option<String>,
    pub lineas: i64,
}

const COLUMNAS: &str = "c.id, c.fecha, c.proveedor_id, p.nombre AS proveedor, c.documento, c.total,
    c.nota, c.pagada_con_caja, c.movimiento_caja_id, c.anulada_en, c.motivo_anulacion,
    (SELECT COUNT(*) FROM compra_items i WHERE i.compra_id = c.id) AS lineas";

pub async fn por_id(conn: &mut SqliteConnection, id: i64) -> Resultado<Option<FilaCompra>> {
    let sql = format!(
        "SELECT {COLUMNAS} FROM compras c LEFT JOIN proveedores p ON p.id = c.proveedor_id
         WHERE c.id = ?"
    );
    Ok(sqlx::query_as(&sql).bind(id).fetch_optional(conn).await?)
}

#[derive(Debug, Clone, Default)]
pub struct FiltroCompras {
    pub proveedor_id: Option<i64>,
    pub desde_utc: Option<String>,
    pub hasta_utc: Option<String>,
}

/// De la más nueva a la más antigua (idx_compras_fecha o idx_compras_proveedor).
pub async fn listar(
    pool: &SqlitePool,
    f: &FiltroCompras,
    limite: i64,
    desplazamiento: i64,
) -> Resultado<Vec<FilaCompra>> {
    let mut q = QueryBuilder::<Sqlite>::new(format!(
        "SELECT {COLUMNAS} FROM compras c LEFT JOIN proveedores p ON p.id = c.proveedor_id
         WHERE 1 = 1"
    ));
    if let Some(id) = f.proveedor_id {
        q.push(" AND c.proveedor_id = ").push_bind(id);
    }
    if let Some(d) = &f.desde_utc {
        q.push(" AND c.fecha >= ").push_bind(d.clone());
    }
    if let Some(h) = &f.hasta_utc {
        q.push(" AND c.fecha < ").push_bind(h.clone());
    }
    q.push(" ORDER BY c.fecha DESC, c.id DESC LIMIT ")
        .push_bind(limite)
        .push(" OFFSET ")
        .push_bind(desplazamiento);
    Ok(q.build_query_as().fetch_all(pool).await?)
}

#[derive(Debug, Clone, FromRow)]
pub struct FilaItemCompra {
    pub producto_id: i64,
    pub nombre: String,
    pub unidad: Unidad,
    pub cantidad_mil: i64,
    pub costo_unitario: i64,
    pub subtotal: i64,
    /// Stock actual del producto (para avisar si anular lo dejaría negativo).
    pub stock_mil: i64,
}

pub async fn items(conn: &mut SqliteConnection, compra_id: i64) -> Resultado<Vec<FilaItemCompra>> {
    Ok(sqlx::query_as(
        "SELECT i.producto_id, p.nombre, p.unidad, i.cantidad_mil, i.costo_unitario, i.subtotal,
                p.stock_mil
         FROM compra_items i JOIN productos p ON p.id = i.producto_id
         WHERE i.compra_id = ? ORDER BY i.id",
    )
    .bind(compra_id)
    .fetch_all(conn)
    .await?)
}
