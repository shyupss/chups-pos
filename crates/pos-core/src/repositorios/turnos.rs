use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::Resultado;

#[derive(Debug, Clone, FromRow)]
pub struct FilaTurno {
    pub id: i64,
    pub abierto_en: String,
    pub cerrado_en: Option<String>,
    pub monto_inicial: i64,
    pub efectivo_esperado: Option<i64>,
    pub efectivo_contado: Option<i64>,
    pub diferencia: Option<i64>,
    pub detalle_conteo: Option<String>,
    pub nota_cierre: Option<String>,
    pub estado: String,
}

const COLUMNAS: &str = "id, abierto_en, cerrado_en, monto_inicial, efectivo_esperado, \
                        efectivo_contado, diferencia, detalle_conteo, nota_cierre, estado";

/// Id del turno abierto. Usa el índice único parcial: es la consulta de cada venta.
pub async fn id_abierto(conn: &mut SqliteConnection) -> Resultado<Option<i64>> {
    Ok(
        sqlx::query_scalar("SELECT id FROM turnos WHERE estado = 'abierto'")
            .fetch_optional(conn)
            .await?,
    )
}

pub async fn abierto(conn: &mut SqliteConnection) -> Resultado<Option<FilaTurno>> {
    let sql = format!("SELECT {COLUMNAS} FROM turnos WHERE estado = 'abierto'");
    Ok(sqlx::query_as(&sql).fetch_optional(conn).await?)
}

pub async fn por_id(conn: &mut SqliteConnection, id: i64) -> Resultado<Option<FilaTurno>> {
    let sql = format!("SELECT {COLUMNAS} FROM turnos WHERE id = ?");
    Ok(sqlx::query_as(&sql).bind(id).fetch_optional(conn).await?)
}

/// Monto inicial del último turno, para sugerirlo al abrir.
pub async fn ultimo_monto_inicial(conn: &mut SqliteConnection) -> Resultado<Option<i64>> {
    Ok(
        sqlx::query_scalar("SELECT monto_inicial FROM turnos ORDER BY id DESC LIMIT 1")
            .fetch_optional(conn)
            .await?,
    )
}

/// Abre un turno. Si ya hay uno abierto, el índice único lo rechaza.
pub async fn insertar(conn: &mut SqliteConnection, monto_inicial: i64) -> Resultado<FilaTurno> {
    let sql = format!("INSERT INTO turnos (monto_inicial) VALUES (?) RETURNING {COLUMNAS}");
    Ok(sqlx::query_as(&sql)
        .bind(monto_inicial)
        .fetch_one(conn)
        .await?)
}

pub struct Cierre<'a> {
    pub esperado: i64,
    pub contado: i64,
    pub detalle_conteo: Option<&'a str>,
    pub nota: Option<&'a str>,
}

/// Cierra el turno si sigue abierto. Devuelve `false` si otro lo cerró antes.
pub async fn cerrar(conn: &mut SqliteConnection, id: i64, c: &Cierre<'_>) -> Resultado<bool> {
    let r = sqlx::query(
        "UPDATE turnos
         SET estado = 'cerrado', cerrado_en = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             efectivo_esperado = ?, efectivo_contado = ?, diferencia = ?,
             detalle_conteo = ?, nota_cierre = ?
         WHERE id = ? AND estado = 'abierto'",
    )
    .bind(c.esperado)
    .bind(c.contado)
    .bind(c.contado - c.esperado)
    .bind(c.detalle_conteo)
    .bind(c.nota)
    .bind(id)
    .execute(conn)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// Turnos del más nuevo al más antiguo, para el filtro de la lista de ventas.
pub async fn listar(
    pool: &SqlitePool,
    limite: i64,
    desplazamiento: i64,
) -> Resultado<Vec<FilaTurno>> {
    let sql = format!("SELECT {COLUMNAS} FROM turnos ORDER BY id DESC LIMIT ? OFFSET ?");
    Ok(sqlx::query_as(&sql)
        .bind(limite)
        .bind(desplazamiento)
        .fetch_all(pool)
        .await?)
}

#[derive(Debug, Clone, FromRow)]
pub struct FilaMovimiento {
    pub id: i64,
    pub tipo: String,
    pub monto: i64,
    pub motivo: String,
    pub fecha: String,
}

pub async fn insertar_movimiento(
    conn: &mut SqliteConnection,
    turno_id: i64,
    tipo: &str,
    monto: i64,
    motivo: &str,
) -> Resultado<FilaMovimiento> {
    Ok(sqlx::query_as(
        "INSERT INTO movimientos_caja (turno_id, tipo, monto, motivo) VALUES (?, ?, ?, ?)
         RETURNING id, tipo, monto, motivo, fecha",
    )
    .bind(turno_id)
    .bind(tipo)
    .bind(monto)
    .bind(motivo)
    .fetch_one(conn)
    .await?)
}

pub async fn movimientos(
    conn: &mut SqliteConnection,
    turno_id: i64,
) -> Resultado<Vec<FilaMovimiento>> {
    Ok(sqlx::query_as(
        "SELECT id, tipo, monto, motivo, fecha FROM movimientos_caja
         WHERE turno_id = ? ORDER BY id",
    )
    .bind(turno_id)
    .fetch_all(conn)
    .await?)
}

/// Cantidad y monto agrupados por medio de pago (o de reembolso).
#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct PorMedio {
    pub medio: String,
    pub cantidad: i64,
    pub monto: i64,
    /// Ajuste a la decena incluido en `monto`.
    pub redondeo: i64,
}

/// Ventas del turno por medio de pago, incluidas las que después se anularon: el dinero entró.
pub async fn ventas_por_medio(
    conn: &mut SqliteConnection,
    turno_id: i64,
) -> Resultado<Vec<PorMedio>> {
    Ok(sqlx::query_as(
        "SELECT medio_pago AS medio, COUNT(*) AS cantidad, SUM(total) AS monto,
                SUM(redondeo) AS redondeo
         FROM ventas WHERE turno_id = ? GROUP BY medio_pago",
    )
    .bind(turno_id)
    .fetch_all(conn)
    .await?)
}

pub async fn abonos_por_medio(
    conn: &mut SqliteConnection,
    turno_id: i64,
) -> Resultado<Vec<PorMedio>> {
    Ok(sqlx::query_as(
        "SELECT medio, COUNT(*) AS cantidad, SUM(monto) AS monto, SUM(redondeo) AS redondeo
         FROM (
             -- Pagos de clientes: en efectivo entró monto + redondeo.
             SELECT medio_pago AS medio, monto + redondeo AS monto, redondeo
             FROM pagos_cliente WHERE turno_id = ?1
             UNION ALL
             -- Abonos de antes de los pagos (sin pago ni traslado): solo dinero que entró.
             SELECT medio_pago, monto, 0 FROM abonos
             WHERE turno_id = ?1 AND pago_id IS NULL AND traslado_de_venta_id IS NULL
         ) GROUP BY medio",
    )
    .bind(turno_id)
    .fetch_all(conn)
    .await?)
}

/// Ventas anuladas durante el turno, por medio de reembolso ('fiado' = sin reembolso).
pub async fn anulaciones_por_medio(
    conn: &mut SqliteConnection,
    turno_id: i64,
) -> Resultado<Vec<PorMedio>> {
    Ok(sqlx::query_as(
        "SELECT COALESCE(medio_reembolso_anulacion, 'fiado') AS medio, COUNT(*) AS cantidad,
                SUM(COALESCE(reembolso_anulacion, total)) AS monto,
                SUM(COALESCE(redondeo_anulacion, 0)) AS redondeo
         FROM ventas WHERE turno_anulacion_id = ? GROUP BY 1",
    )
    .bind(turno_id)
    .fetch_all(conn)
    .await?)
}

/// Devoluciones hechas durante el turno, por medio de reembolso ('fiado' = sin reembolso).
pub async fn devoluciones_por_medio(
    conn: &mut SqliteConnection,
    turno_id: i64,
) -> Resultado<Vec<PorMedio>> {
    Ok(sqlx::query_as(
        "SELECT medio, COUNT(*) AS cantidad, SUM(monto) AS monto, SUM(redondeo) AS redondeo
         FROM (
             -- Lo que salió en dinero.
             SELECT medio_reembolso AS medio, monto - rebaja_deuda AS monto, redondeo
             FROM devoluciones WHERE turno_id = ?1 AND medio_reembolso IS NOT NULL
             UNION ALL
             -- Lo que bajó deudas de fiados (o pasó como saldo a favor): no es efectivo.
             SELECT 'fiado', rebaja_deuda, 0
             FROM devoluciones WHERE turno_id = ?1 AND rebaja_deuda > 0
         ) GROUP BY medio",
    )
    .bind(turno_id)
    .fetch_all(conn)
    .await?)
}

/// Pagos de clientes anulados durante el turno, por medio (el dinero salió de este turno).
pub async fn pagos_anulados_por_medio(
    conn: &mut SqliteConnection,
    turno_id: i64,
) -> Resultado<Vec<PorMedio>> {
    Ok(sqlx::query_as(
        "SELECT medio_pago AS medio, COUNT(*) AS cantidad, SUM(monto + redondeo) AS monto,
                SUM(redondeo) AS redondeo
         FROM pagos_cliente WHERE turno_anulacion_id = ? GROUP BY medio_pago",
    )
    .bind(turno_id)
    .fetch_all(conn)
    .await?)
}
