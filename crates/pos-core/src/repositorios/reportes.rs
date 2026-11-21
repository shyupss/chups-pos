//! Consultas de los reportes. Todas reciben límites UTC `[inicio, fin)` ya calculados (ver
//! `zona::limites`) para filtrar por los índices de fecha, y agrupan lo justo para que Rust arme
//! días y horas locales. Las ventas anuladas nunca cuentan; las devoluciones se descuentan de la
//! venta a la que pertenecen.

use sqlx::{FromRow, SqliteConnection};

use crate::repositorios::fiados::SALDO;
use crate::Resultado;

/// Ventas de una hora UTC ("2026-11-21T14") y un medio de pago, ya sin lo devuelto.
#[derive(Debug, Clone, FromRow)]
pub struct FilaHora {
    pub hora: String,
    pub medio_pago: String,
    pub boletas: i64,
    pub monto: i64,
}

pub async fn ventas_por_hora(
    conn: &mut SqliteConnection,
    inicio: &str,
    fin: &str,
) -> Resultado<Vec<FilaHora>> {
    Ok(sqlx::query_as(
        "SELECT substr(fecha_hora, 1, 13) AS hora, medio_pago, COUNT(*) AS boletas,
                SUM(total - monto_devuelto) AS monto
         FROM ventas
         WHERE fecha_hora >= ? AND fecha_hora < ? AND estado <> 'anulada'
         GROUP BY hora, medio_pago",
    )
    .bind(inicio)
    .bind(fin)
    .fetch_all(conn)
    .await?)
}

/// Lo vendido de un producto (o lo devuelto, según la consulta). El costo de cada línea es
/// costo × cantidad redondeado al peso; las líneas sin costo no suman costo ni "con costo".
#[derive(Debug, Clone, Default, FromRow)]
pub struct FilaProducto {
    pub producto_id: i64,
    pub nombre: String,
    pub unidad: String,
    pub categoria_id: Option<i64>,
    pub cantidad_mil: i64,
    pub monto: i64,
    /// Parte del monto de líneas que tienen costo.
    pub monto_con_costo: i64,
    pub costo: i64,
    /// Parte del monto y del costo que viene de costos estimados (ventas antes de la fase 6).
    pub monto_estimado: i64,
    pub costo_estimado: i64,
}

const COSTO_LINEA: &str = "(i.costo_unitario * {cantidad} + 500) / 1000";

fn columnas(cantidad: &str, monto: &str) -> String {
    let costo = COSTO_LINEA.replace("{cantidad}", cantidad);
    format!(
        "i.producto_id, p.nombre, p.unidad, p.categoria_id,
         SUM({cantidad}) AS cantidad_mil,
         SUM({monto}) AS monto,
         SUM(CASE WHEN i.costo_unitario IS NULL THEN 0 ELSE {monto} END) AS monto_con_costo,
         SUM(CASE WHEN i.costo_unitario IS NULL THEN 0 ELSE {costo} END) AS costo,
         SUM(CASE WHEN i.costo_estimado = 1 THEN {monto} ELSE 0 END) AS monto_estimado,
         SUM(CASE WHEN i.costo_estimado = 1 THEN {costo} ELSE 0 END) AS costo_estimado"
    )
}

/// Lo vendido por producto en las ventas del rango (sin descontar devoluciones). CROSS JOIN fija
/// el orden ventas (por idx_ventas_fecha) → líneas → producto: si no, SQLite puede empezar por
/// los productos, que son pocos, y recorrer todas las líneas de la historia.
pub async fn vendido_por_producto(
    conn: &mut SqliteConnection,
    inicio: &str,
    fin: &str,
) -> Resultado<Vec<FilaProducto>> {
    let sql = format!(
        "SELECT {}
         FROM ventas v
         CROSS JOIN venta_items i ON i.venta_id = v.id
         CROSS JOIN productos p ON p.id = i.producto_id
         WHERE v.fecha_hora >= ? AND v.fecha_hora < ? AND v.estado <> 'anulada'
         GROUP BY i.producto_id",
        columnas("i.cantidad_mil", "i.subtotal")
    );
    Ok(sqlx::query_as(&sql)
        .bind(inicio)
        .bind(fin)
        .fetch_all(conn)
        .await?)
}

/// Lo devuelto por producto de las ventas del rango. Empieza por las devoluciones (pocas) y no
/// por las ventas: CROSS JOIN fija ese orden en SQLite.
pub async fn devuelto_por_producto(
    conn: &mut SqliteConnection,
    inicio: &str,
    fin: &str,
) -> Resultado<Vec<FilaProducto>> {
    let sql = format!(
        "SELECT {}
         FROM devolucion_items di
         CROSS JOIN venta_items i ON i.id = di.venta_item_id
         CROSS JOIN ventas v ON v.id = i.venta_id
         CROSS JOIN productos p ON p.id = i.producto_id
         WHERE v.fecha_hora >= ? AND v.fecha_hora < ? AND v.estado <> 'anulada'
         GROUP BY i.producto_id",
        columnas("di.cantidad_mil", "di.monto")
    );
    Ok(sqlx::query_as(&sql)
        .bind(inicio)
        .bind(fin)
        .fetch_all(conn)
        .await?)
}

#[derive(Debug, Clone, FromRow)]
pub struct FilaCatalogo {
    pub id: i64,
    pub nombre: String,
    pub unidad: String,
    pub categoria_id: Option<i64>,
    pub stock_mil: i64,
    pub activo: bool,
}

/// Todos los productos activos (para "sin ventas en el rango").
pub async fn productos_activos(conn: &mut SqliteConnection) -> Resultado<Vec<FilaCatalogo>> {
    Ok(sqlx::query_as(
        "SELECT id, nombre, unidad, categoria_id, stock_mil, activo FROM productos
         WHERE activo = 1 ORDER BY nombre COLLATE NOCASE",
    )
    .fetch_all(conn)
    .await?)
}

pub async fn categorias(conn: &mut SqliteConnection) -> Resultado<Vec<(i64, String)>> {
    Ok(sqlx::query_as("SELECT id, nombre FROM categorias")
        .fetch_all(conn)
        .await?)
}

/// Pagos de fiados recibidos en el rango (sin los anulados), más los abonos sueltos de antes de
/// los pagos (fase 4). Los traslados de saldo a favor no son dinero que entra.
pub async fn pagos_fiado(conn: &mut SqliteConnection, inicio: &str, fin: &str) -> Resultado<i64> {
    let pagos: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(monto), 0) FROM pagos_cliente
         WHERE fecha >= ? AND fecha < ? AND anulado_en IS NULL",
    )
    .bind(inicio)
    .bind(fin)
    .fetch_one(&mut *conn)
    .await?;
    let sueltos: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(monto), 0) FROM abonos
         WHERE pago_id IS NULL AND traslado_de_venta_id IS NULL AND fecha >= ? AND fecha < ?",
    )
    .bind(inicio)
    .bind(fin)
    .fetch_one(conn)
    .await?;
    Ok(pagos + sueltos)
}

#[derive(Debug, Clone, FromRow)]
pub struct FilaBajoMinimo {
    pub id: i64,
    pub nombre: String,
    pub unidad: String,
    pub stock_mil: i64,
    pub stock_minimo_mil: i64,
}

/// Los productos más bajo su mínimo (por el índice parcial de la fase 5).
pub async fn bajo_minimo(
    conn: &mut SqliteConnection,
    limite: i64,
) -> Resultado<Vec<FilaBajoMinimo>> {
    Ok(sqlx::query_as(
        "SELECT id, nombre, unidad, stock_mil, stock_minimo_mil FROM productos
         WHERE activo = 1 AND stock_minimo_mil > 0 AND stock_mil - stock_minimo_mil <= 0
         ORDER BY stock_mil - stock_minimo_mil LIMIT ?",
    )
    .bind(limite)
    .fetch_all(conn)
    .await?)
}

#[derive(Debug, Clone, FromRow)]
pub struct FilaAtrasado {
    pub id: i64,
    pub nombre: String,
    pub desde: String,
    pub deuda: i64,
}

/// Clientes cuya venta fiada pendiente más antigua es anterior a `antes_de`.
pub async fn clientes_atrasados(
    conn: &mut SqliteConnection,
    antes_de: &str,
    limite: i64,
) -> Resultado<Vec<FilaAtrasado>> {
    let sql = format!(
        "SELECT c.id, c.nombre, MIN(v.fecha_hora) AS desde, SUM({SALDO}) AS deuda
         FROM ventas v JOIN clientes c ON c.id = v.cliente_id
         WHERE v.estado = 'fiada'
         GROUP BY c.id HAVING MIN(v.fecha_hora) < ?
         ORDER BY desde LIMIT ?"
    );
    Ok(sqlx::query_as(&sql)
        .bind(antes_de)
        .bind(limite)
        .fetch_all(conn)
        .await?)
}

#[derive(Debug, Clone, FromRow)]
pub struct FilaTurnoCerrado {
    pub id: i64,
    pub abierto_en: String,
    pub cerrado_en: String,
    pub monto_inicial: i64,
    pub efectivo_esperado: i64,
    pub efectivo_contado: i64,
    pub diferencia: i64,
}

/// Turnos cerrados dentro del rango (por la fecha de cierre).
pub async fn turnos_cerrados(
    conn: &mut SqliteConnection,
    inicio: &str,
    fin: &str,
) -> Resultado<Vec<FilaTurnoCerrado>> {
    Ok(sqlx::query_as(
        "SELECT id, abierto_en, cerrado_en, monto_inicial, efectivo_esperado, efectivo_contado,
                diferencia
         FROM turnos
         WHERE estado = 'cerrado' AND cerrado_en >= ? AND cerrado_en < ?
         ORDER BY cerrado_en, id",
    )
    .bind(inicio)
    .bind(fin)
    .fetch_all(conn)
    .await?)
}

/// Una venta fiada con lo que se debía de ella en el instante `hasta`.
#[derive(Debug, Clone, FromRow)]
pub struct FilaDeudaEn {
    pub cliente_id: i64,
    pub nombre: String,
    pub fecha_hora: String,
    pub saldo: i64,
}

/// Lo que se debía de cada venta fiada en el instante `hasta`: lo devuelto, abonado y
/// trasladado hasta ese momento. Las anuladas y los pagos anulados no cuentan nunca (fueron
/// errores), igual que en los demás reportes.
pub async fn deuda_en(conn: &mut SqliteConnection, hasta: &str) -> Resultado<Vec<FilaDeudaEn>> {
    Ok(sqlx::query_as(
        "SELECT v.cliente_id, c.nombre, v.fecha_hora,
                MAX(0, v.total
                    - COALESCE((SELECT SUM(d.rebaja_deuda) FROM devoluciones d
                                WHERE d.venta_id = v.id AND d.fecha < ?1), 0)
                    - COALESCE((SELECT SUM(a.monto) FROM abonos a
                                LEFT JOIN pagos_cliente p ON p.id = a.pago_id
                                WHERE a.venta_id = v.id AND a.fecha < ?1
                                  AND p.anulado_en IS NULL), 0)
                    + COALESCE((SELECT SUM(t.monto) FROM abonos t
                                WHERE t.traslado_de_venta_id = v.id AND t.fecha < ?1), 0)) AS saldo
         FROM ventas v JOIN clientes c ON c.id = v.cliente_id
         WHERE v.cliente_id IS NOT NULL AND v.medio_pago = 'fiado' AND v.estado <> 'anulada'
           AND v.fecha_hora < ?1",
    )
    .bind(hasta)
    .fetch_all(conn)
    .await?)
}
