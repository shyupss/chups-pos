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

#[derive(Debug, Clone, FromRow)]
pub struct FilaDeudor {
    pub id: i64,
    pub nombre: String,
    pub telefono: Option<String>,
    pub limite_credito: Option<i64>,
    pub activo: bool,
    pub deuda: i64,
    pub fiada_mas_antigua: Option<String>,
    pub ultimo_pago: Option<String>,
}

/// Clientes con deuda (o todos), de la deuda mayor a la menor.
pub async fn deudores(
    conn: &mut SqliteConnection,
    texto: Option<&str>,
    todos: bool,
    limite: i64,
    desplazamiento: i64,
) -> Resultado<Vec<FilaDeudor>> {
    let sql = format!(
        "SELECT c.id, c.nombre, c.telefono, c.limite_credito, c.activo,
                COALESCE((SELECT SUM({SALDO}) FROM ventas v
                          WHERE v.cliente_id = c.id AND v.estado = 'fiada'), 0) AS deuda,
                (SELECT MIN(v.fecha_hora) FROM ventas v
                 WHERE v.cliente_id = c.id AND v.estado = 'fiada') AS fiada_mas_antigua,
                (SELECT MAX(p.fecha) FROM pagos_cliente p
                 WHERE p.cliente_id = c.id AND p.anulado_en IS NULL) AS ultimo_pago
         FROM clientes c
         WHERE (?1 OR EXISTS (SELECT 1 FROM ventas v WHERE v.cliente_id = c.id AND v.estado = 'fiada'))
           AND (?2 IS NULL OR c.nombre LIKE '%' || ?2 || '%' OR c.telefono LIKE '%' || ?2 || '%')
         ORDER BY deuda DESC, c.nombre COLLATE NOCASE, c.id
         LIMIT ?3 OFFSET ?4"
    );
    Ok(sqlx::query_as(&sql)
        .bind(todos)
        .bind(texto)
        .bind(limite)
        .bind(desplazamiento)
        .fetch_all(conn)
        .await?)
}

/// Lo que se le debe al negocio y cuántos clientes deben.
pub async fn total_adeudado(conn: &mut SqliteConnection) -> Resultado<(i64, i64)> {
    let sql = format!(
        "SELECT COALESCE(SUM({SALDO}), 0), COUNT(DISTINCT v.cliente_id)
         FROM ventas v WHERE v.estado = 'fiada'"
    );
    Ok(sqlx::query_as(&sql).fetch_one(conn).await?)
}

pub struct NuevoPago<'a> {
    pub cliente_id: i64,
    pub monto: i64,
    pub redondeo: i64,
    pub monto_recibido: Option<i64>,
    pub vuelto: Option<i64>,
    pub medio_pago: &'a str,
    pub turno_id: Option<i64>,
    pub nota: Option<&'a str>,
}

pub async fn insertar_pago(conn: &mut SqliteConnection, p: &NuevoPago<'_>) -> Resultado<i64> {
    Ok(sqlx::query_scalar(
        "INSERT INTO pagos_cliente
            (cliente_id, monto, redondeo, monto_recibido, vuelto, medio_pago, turno_id, nota)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(p.cliente_id)
    .bind(p.monto)
    .bind(p.redondeo)
    .bind(p.monto_recibido)
    .bind(p.vuelto)
    .bind(p.medio_pago)
    .bind(p.turno_id)
    .bind(p.nota)
    .fetch_one(conn)
    .await?)
}

#[derive(Debug, Clone, FromRow)]
pub struct FilaPago {
    pub id: i64,
    pub cliente_id: i64,
    pub fecha: String,
    pub monto: i64,
    pub redondeo: i64,
    pub monto_recibido: Option<i64>,
    pub vuelto: Option<i64>,
    pub medio_pago: String,
    pub turno_id: Option<i64>,
    pub nota: Option<String>,
    pub anulado_en: Option<String>,
    pub motivo_anulacion: Option<String>,
}

const COLUMNAS_PAGO: &str = "id, cliente_id, fecha, monto, redondeo, monto_recibido, vuelto,
    medio_pago, turno_id, nota, anulado_en, motivo_anulacion";

pub async fn pago(conn: &mut SqliteConnection, id: i64) -> Resultado<Option<FilaPago>> {
    let sql = format!("SELECT {COLUMNAS_PAGO} FROM pagos_cliente WHERE id = ?");
    Ok(sqlx::query_as(&sql).bind(id).fetch_optional(conn).await?)
}

/// Pagos del cliente, del más nuevo al más antiguo.
pub async fn pagos(
    conn: &mut SqliteConnection,
    cliente_id: i64,
    limite: i64,
) -> Resultado<Vec<FilaPago>> {
    let sql = format!(
        "SELECT {COLUMNAS_PAGO} FROM pagos_cliente WHERE cliente_id = ?
         ORDER BY fecha DESC, id DESC LIMIT ?"
    );
    Ok(sqlx::query_as(&sql)
        .bind(cliente_id)
        .bind(limite)
        .fetch_all(conn)
        .await?)
}

#[derive(Debug, Clone, FromRow)]
pub struct FilaAbonoPago {
    pub venta_id: i64,
    pub folio: i64,
    pub monto: i64,
    pub estado: String,
    /// La venta fue anulada, trasladó saldo a favor o devolvió dinero: el pago ya no se puede
    /// deshacer sin descuadrar.
    pub bloquea: bool,
}

/// Abonos de un pago, con la venta que pagó.
pub async fn abonos_de_pago(
    conn: &mut SqliteConnection,
    pago_id: i64,
) -> Resultado<Vec<FilaAbonoPago>> {
    Ok(sqlx::query_as(
        "SELECT a.venta_id, v.folio, a.monto, v.estado,
                (v.estado = 'anulada'
                 OR EXISTS (SELECT 1 FROM abonos t WHERE t.traslado_de_venta_id = v.id)
                 OR EXISTS (SELECT 1 FROM devoluciones d
                            WHERE d.venta_id = v.id AND d.medio_reembolso IS NOT NULL)) AS bloquea
         FROM abonos a JOIN ventas v ON v.id = a.venta_id
         WHERE a.pago_id = ? ORDER BY a.id",
    )
    .bind(pago_id)
    .fetch_all(conn)
    .await?)
}

/// Marca el pago como anulado si no lo estaba. `false` si ya lo estaba.
pub async fn anular_pago(
    conn: &mut SqliteConnection,
    id: i64,
    motivo: &str,
    turno_id: Option<i64>,
) -> Resultado<bool> {
    let r = sqlx::query(
        "UPDATE pagos_cliente
         SET anulado_en = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), motivo_anulacion = ?,
             turno_anulacion_id = ?
         WHERE id = ? AND anulado_en IS NULL",
    )
    .bind(motivo)
    .bind(turno_id)
    .bind(id)
    .execute(conn)
    .await?;
    Ok(r.rows_affected() == 1)
}
