use sqlx::{FromRow, SqliteConnection};

use crate::repositorios::productos::Unidad;
use crate::Resultado;

#[derive(Debug, Clone, FromRow)]
pub struct FilaDevolucion {
    pub id: i64,
    pub turno_id: Option<i64>,
    pub fecha: String,
    pub monto: i64,
    pub redondeo: i64,
    pub medio_reembolso: Option<String>,
    pub motivo: String,
}

/// Devoluciones de una venta, de la primera a la última.
pub async fn de_venta(
    conn: &mut SqliteConnection,
    venta_id: i64,
) -> Resultado<Vec<FilaDevolucion>> {
    Ok(sqlx::query_as(
        "SELECT id, turno_id, fecha, monto, redondeo, medio_reembolso, motivo
         FROM devoluciones WHERE venta_id = ? ORDER BY id",
    )
    .bind(venta_id)
    .fetch_all(conn)
    .await?)
}

#[derive(Debug, Clone, FromRow)]
pub struct FilaDevuelta {
    pub devolucion_id: i64,
    pub venta_item_id: i64,
    pub nombre: String,
    pub unidad: Unidad,
    pub precio_unitario: i64,
    pub cantidad_mil: i64,
    pub monto: i64,
    pub vuelve_a_stock: bool,
}

/// Líneas devueltas de todas las devoluciones de una venta.
pub async fn items_de_venta(
    conn: &mut SqliteConnection,
    venta_id: i64,
) -> Resultado<Vec<FilaDevuelta>> {
    Ok(sqlx::query_as(
        "SELECT di.devolucion_id, di.venta_item_id, vi.nombre, vi.unidad, vi.precio_unitario,
                di.cantidad_mil, di.monto, di.vuelve_a_stock
         FROM devolucion_items di
         JOIN devoluciones d ON d.id = di.devolucion_id
         JOIN venta_items vi ON vi.id = di.venta_item_id
         WHERE d.venta_id = ? ORDER BY di.id",
    )
    .bind(venta_id)
    .fetch_all(conn)
    .await?)
}

pub struct NuevaDevolucion<'a> {
    pub venta_id: i64,
    pub turno_id: Option<i64>,
    pub monto: i64,
    pub redondeo: i64,
    pub medio_reembolso: Option<&'a str>,
    pub motivo: &'a str,
}

pub async fn insertar(conn: &mut SqliteConnection, d: &NuevaDevolucion<'_>) -> Resultado<i64> {
    Ok(sqlx::query_scalar(
        "INSERT INTO devoluciones (venta_id, turno_id, monto, redondeo, medio_reembolso, motivo)
         VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(d.venta_id)
    .bind(d.turno_id)
    .bind(d.monto)
    .bind(d.redondeo)
    .bind(d.medio_reembolso)
    .bind(d.motivo)
    .fetch_one(conn)
    .await?)
}

pub async fn insertar_item(
    conn: &mut SqliteConnection,
    devolucion_id: i64,
    venta_item_id: i64,
    cantidad_mil: i64,
    monto: i64,
    vuelve_a_stock: bool,
) -> Resultado<()> {
    sqlx::query(
        "INSERT INTO devolucion_items
            (devolucion_id, venta_item_id, cantidad_mil, monto, vuelve_a_stock)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(devolucion_id)
    .bind(venta_item_id)
    .bind(cantidad_mil)
    .bind(monto)
    .bind(vuelve_a_stock)
    .execute(conn)
    .await?;
    Ok(())
}

/// La venta de una devolución.
pub async fn venta_de(conn: &mut SqliteConnection, devolucion_id: i64) -> Resultado<Option<i64>> {
    Ok(
        sqlx::query_scalar("SELECT venta_id FROM devoluciones WHERE id = ?")
            .bind(devolucion_id)
            .fetch_optional(conn)
            .await?,
    )
}
