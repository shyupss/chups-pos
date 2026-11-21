use sqlx::{FromRow, SqliteConnection};

use crate::Resultado;

/// Saldo de la venta fiada `v`: total − lo que bajaron sus devoluciones − sus abonos vigentes
/// (los de pagos no anulados) + lo que se trasladó desde ella a otras deudas como saldo a favor.
/// Usa los índices por venta de devoluciones y abonos, y el parcial de traslados.
pub const SALDO: &str = "(v.total
    - COALESCE((SELECT SUM(d.rebaja_deuda) FROM devoluciones d WHERE d.venta_id = v.id), 0)
    - COALESCE((SELECT SUM(a.monto) FROM abonos a LEFT JOIN pagos_cliente p ON p.id = a.pago_id
                WHERE a.venta_id = v.id AND p.anulado_en IS NULL), 0)
    + COALESCE((SELECT SUM(t.monto) FROM abonos t WHERE t.traslado_de_venta_id = v.id), 0))";

/// Lo abonado a la venta `v` (pagos vigentes y saldo a favor que llegó de otras ventas).
pub const ABONADO: &str = "COALESCE((SELECT SUM(a.monto) FROM abonos a
    LEFT JOIN pagos_cliente p ON p.id = a.pago_id
    WHERE a.venta_id = v.id AND p.anulado_en IS NULL), 0)";

#[derive(Debug, Clone, FromRow)]
pub struct FilaFiada {
    pub venta_id: i64,
    pub cliente_id: Option<i64>,
    pub folio: i64,
    pub fecha_hora: String,
    pub total: i64,
    pub devuelto: i64,
    pub abonado: i64,
    pub saldo: i64,
    pub estado: String,
    pub pagada_en: Option<String>,
}

fn select() -> String {
    format!(
        "SELECT v.id AS venta_id, v.cliente_id, v.folio, v.fecha_hora, v.total,
                COALESCE((SELECT SUM(d.rebaja_deuda) FROM devoluciones d WHERE d.venta_id = v.id), 0)
                    AS devuelto,
                {ABONADO} AS abonado, {SALDO} AS saldo, v.estado, v.pagada_en
         FROM ventas v"
    )
}

/// Ventas fiadas pendientes del cliente, de la más antigua a la más nueva (así se pagan).
pub async fn pendientes(conn: &mut SqliteConnection, cliente_id: i64) -> Resultado<Vec<FilaFiada>> {
    let sql = format!(
        "{} WHERE v.cliente_id = ? AND v.estado = 'fiada' ORDER BY v.fecha_hora, v.id",
        select()
    );
    Ok(sqlx::query_as(&sql)
        .bind(cliente_id)
        .fetch_all(conn)
        .await?)
}

/// Ventas fiadas ya saldadas, de la más nueva a la más antigua.
pub async fn pagadas(
    conn: &mut SqliteConnection,
    cliente_id: i64,
    limite: i64,
) -> Resultado<Vec<FilaFiada>> {
    let sql = format!(
        "{} WHERE v.cliente_id = ? AND v.estado = 'pagada' AND v.medio_pago = 'fiado'
         ORDER BY v.pagada_en DESC, v.id DESC LIMIT ?",
        select()
    );
    Ok(sqlx::query_as(&sql)
        .bind(cliente_id)
        .bind(limite)
        .fetch_all(conn)
        .await?)
}

pub async fn venta(conn: &mut SqliteConnection, venta_id: i64) -> Resultado<Option<FilaFiada>> {
    let sql = format!("{} WHERE v.id = ?", select());
    Ok(sqlx::query_as(&sql)
        .bind(venta_id)
        .fetch_optional(conn)
        .await?)
}

/// Deuda total del cliente: la suma de los saldos de sus fiadas pendientes.
pub async fn deuda(conn: &mut SqliteConnection, cliente_id: i64) -> Resultado<i64> {
    let sql = format!(
        "SELECT COALESCE(SUM({SALDO}), 0) FROM ventas v
         WHERE v.cliente_id = ? AND v.estado = 'fiada'"
    );
    Ok(sqlx::query_scalar(&sql)
        .bind(cliente_id)
        .fetch_one(conn)
        .await?)
}

pub struct NuevoAbono<'a> {
    pub venta_id: i64,
    pub monto: i64,
    pub medio_pago: &'a str,
    pub pago_id: Option<i64>,
    pub traslado_de_venta_id: Option<i64>,
    pub turno_id: Option<i64>,
}

pub async fn insertar_abono(conn: &mut SqliteConnection, a: &NuevoAbono<'_>) -> Resultado<i64> {
    Ok(sqlx::query_scalar(
        "INSERT INTO abonos (venta_id, monto, medio_pago, pago_id, traslado_de_venta_id, turno_id)
         VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(a.venta_id)
    .bind(a.monto)
    .bind(a.medio_pago)
    .bind(a.pago_id)
    .bind(a.traslado_de_venta_id)
    .bind(a.turno_id)
    .fetch_one(conn)
    .await?)
}

/// Una fiada que llegó a saldo cero pasa a 'pagada' (medio_pago sigue 'fiado').
pub async fn marcar_pagada(conn: &mut SqliteConnection, venta_id: i64) -> Resultado<()> {
    sqlx::query(
        "UPDATE ventas SET estado = 'pagada', pagada_en = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ? AND estado = 'fiada' AND medio_pago = 'fiado'",
    )
    .bind(venta_id)
    .execute(conn)
    .await?;
    Ok(())
}

/// Al anular un pago, una venta que vuelve a deber pasa otra vez a 'fiada'.
pub async fn marcar_fiada(conn: &mut SqliteConnection, venta_id: i64) -> Resultado<()> {
    sqlx::query(
        "UPDATE ventas SET estado = 'fiada', pagada_en = NULL
         WHERE id = ? AND estado = 'pagada' AND medio_pago = 'fiado'",
    )
    .bind(venta_id)
    .execute(conn)
    .await?;
    Ok(())
}

/// Medio del último abono de la venta (para el saldo a favor que se traslada desde ella).
pub async fn medio_ultimo_abono(
    conn: &mut SqliteConnection,
    venta_id: i64,
) -> Resultado<Option<String>> {
    Ok(sqlx::query_scalar(
        "SELECT medio_pago FROM abonos WHERE venta_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(venta_id)
    .fetch_optional(conn)
    .await?)
}
