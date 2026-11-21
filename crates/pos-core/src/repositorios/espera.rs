use serde::Serialize;
use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::Resultado;

#[derive(Debug, Clone, Serialize, FromRow, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResumenEspera {
    pub id: i64,
    pub nombre: Option<String>,
    pub creada_en: String,
    /// Líneas guardadas, incluidas las de productos que ya no están activos.
    pub lineas: i64,
    /// Total con los precios actuales de los productos activos.
    pub total_estimado: i64,
}

pub async fn contar(conn: &mut SqliteConnection) -> Resultado<i64> {
    let n = sqlx::query_scalar("SELECT COUNT(*) FROM ventas_espera")
        .fetch_one(conn)
        .await?;
    Ok(n)
}

pub async fn existe(conn: &mut SqliteConnection, id: i64) -> Resultado<bool> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ventas_espera WHERE id = ?")
        .bind(id)
        .fetch_one(conn)
        .await?;
    Ok(n > 0)
}

pub async fn insertar(conn: &mut SqliteConnection, nombre: Option<&str>) -> Resultado<i64> {
    let id = sqlx::query_scalar("INSERT INTO ventas_espera (nombre) VALUES (?) RETURNING id")
        .bind(nombre)
        .fetch_one(conn)
        .await?;
    Ok(id)
}

pub async fn insertar_item(
    conn: &mut SqliteConnection,
    venta_espera_id: i64,
    producto_id: i64,
    cantidad_mil: i64,
    orden: i64,
) -> Resultado<()> {
    sqlx::query(
        "INSERT INTO ventas_espera_items (venta_espera_id, producto_id, cantidad_mil, orden)
         VALUES (?, ?, ?, ?)",
    )
    .bind(venta_espera_id)
    .bind(producto_id)
    .bind(cantidad_mil)
    .bind(orden)
    .execute(conn)
    .await?;
    Ok(())
}

/// Ventas en espera, de la más antigua a la más nueva, con su total estimado.
/// El subtotal de cada línea sigue la regla de siempre: (precio × cantidad_mil + 500) / 1000.
pub async fn listar(pool: &SqlitePool) -> Resultado<Vec<ResumenEspera>> {
    Ok(sqlx::query_as(
        "SELECT e.id, e.nombre, e.creada_en,
                COUNT(i.id) AS lineas,
                COALESCE(SUM(CASE WHEN p.activo = 1
                                  THEN (p.precio_venta * i.cantidad_mil + 500) / 1000
                                  ELSE 0 END), 0) AS total_estimado
         FROM ventas_espera e
         LEFT JOIN ventas_espera_items i ON i.venta_espera_id = e.id
         LEFT JOIN productos p ON p.id = i.producto_id
         GROUP BY e.id
         ORDER BY e.creada_en, e.id",
    )
    .fetch_all(pool)
    .await?)
}

/// (producto_id, cantidad_mil) de una venta en espera, en el orden del carrito.
pub async fn items(conn: &mut SqliteConnection, id: i64) -> Resultado<Vec<(i64, i64)>> {
    Ok(sqlx::query_as(
        "SELECT producto_id, cantidad_mil FROM ventas_espera_items
         WHERE venta_espera_id = ? ORDER BY orden",
    )
    .bind(id)
    .fetch_all(conn)
    .await?)
}

/// Borra la venta en espera; sus líneas se van con ON DELETE CASCADE.
pub async fn borrar(conn: &mut SqliteConnection, id: i64) -> Resultado<u64> {
    let r = sqlx::query("DELETE FROM ventas_espera WHERE id = ?")
        .bind(id)
        .execute(conn)
        .await?;
    Ok(r.rows_affected())
}
