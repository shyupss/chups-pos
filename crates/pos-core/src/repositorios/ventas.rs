use sqlx::{FromRow, SqliteConnection};

use crate::repositorios::productos::Unidad;
use crate::Resultado;

pub struct NuevaCabecera<'a> {
    pub subtotal: i64,
    pub descuento: i64,
    /// Ajuste del efectivo a la decena; puede ser negativo.
    pub redondeo: i64,
    pub total: i64,
    pub medio_pago: &'a str,
    pub monto_recibido: Option<i64>,
    pub vuelto: Option<i64>,
    pub estado: &'a str,
    pub cliente_id: Option<i64>,
}

pub struct CabeceraGuardada {
    pub id: i64,
    pub folio: i64,
    pub fecha_hora: String,
}

/// Inserta la cabecera con el siguiente folio correlativo.
/// Debe llamarse dentro de una transacción para que el folio no se repita.
pub async fn insertar_cabecera(
    conn: &mut SqliteConnection,
    v: &NuevaCabecera<'_>,
) -> Resultado<CabeceraGuardada> {
    let (id, folio, fecha_hora): (i64, i64, String) = sqlx::query_as(
        "INSERT INTO ventas
            (folio, subtotal, descuento, redondeo, total, medio_pago, monto_recibido, vuelto,
             estado, cliente_id)
         VALUES ((SELECT COALESCE(MAX(folio), 0) + 1 FROM ventas), ?, ?, ?, ?, ?, ?, ?, ?, ?)
         RETURNING id, folio, fecha_hora",
    )
    .bind(v.subtotal)
    .bind(v.descuento)
    .bind(v.redondeo)
    .bind(v.total)
    .bind(v.medio_pago)
    .bind(v.monto_recibido)
    .bind(v.vuelto)
    .bind(v.estado)
    .bind(v.cliente_id)
    .fetch_one(conn)
    .await?;
    Ok(CabeceraGuardada {
        id,
        folio,
        fecha_hora,
    })
}

/// Línea que se guarda al vender: nombre, unidad y precio quedan copiados (no cambian después).
pub struct NuevoItem<'a> {
    pub producto_id: i64,
    pub nombre: &'a str,
    pub unidad: Unidad,
    pub precio_unitario: i64,
    pub cantidad_mil: i64,
    pub subtotal: i64,
}

pub async fn insertar_item(
    conn: &mut SqliteConnection,
    venta_id: i64,
    item: &NuevoItem<'_>,
) -> Resultado<()> {
    sqlx::query(
        "INSERT INTO venta_items
            (venta_id, producto_id, nombre, unidad, precio_unitario, cantidad_mil, subtotal)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(venta_id)
    .bind(item.producto_id)
    .bind(item.nombre)
    .bind(item.unidad)
    .bind(item.precio_unitario)
    .bind(item.cantidad_mil)
    .bind(item.subtotal)
    .execute(conn)
    .await?;
    Ok(())
}

/// Cabecera de una venta ya guardada, con el nombre del cliente si fue fiada.
#[derive(Debug, Clone, FromRow)]
pub struct CabeceraVenta {
    pub folio: i64,
    pub fecha_hora: String,
    pub subtotal: i64,
    pub descuento: i64,
    pub redondeo: i64,
    pub total: i64,
    pub medio_pago: String,
    pub monto_recibido: Option<i64>,
    pub vuelto: Option<i64>,
    pub cliente: Option<String>,
}

pub async fn cabecera(
    conn: &mut SqliteConnection,
    venta_id: i64,
) -> Resultado<Option<CabeceraVenta>> {
    Ok(sqlx::query_as(
        "SELECT v.folio, v.fecha_hora, v.subtotal, v.descuento, v.redondeo, v.total, v.medio_pago,
                v.monto_recibido, v.vuelto, c.nombre AS cliente
         FROM ventas v LEFT JOIN clientes c ON c.id = v.cliente_id
         WHERE v.id = ?",
    )
    .bind(venta_id)
    .fetch_optional(conn)
    .await?)
}

#[derive(Debug, Clone, FromRow)]
pub struct ItemVendido {
    pub nombre: String,
    pub unidad: Unidad,
    pub precio_unitario: i64,
    pub cantidad_mil: i64,
    pub subtotal: i64,
}

/// Líneas de una venta en el orden en que se cobraron.
pub async fn items(conn: &mut SqliteConnection, venta_id: i64) -> Resultado<Vec<ItemVendido>> {
    Ok(sqlx::query_as(
        "SELECT nombre, unidad, precio_unitario, cantidad_mil, subtotal
         FROM venta_items WHERE venta_id = ? ORDER BY id",
    )
    .bind(venta_id)
    .fetch_all(conn)
    .await?)
}

/// Id de la última venta, para reimprimir su boleta.
pub async fn ultima(conn: &mut SqliteConnection) -> Resultado<Option<i64>> {
    Ok(
        sqlx::query_scalar("SELECT id FROM ventas ORDER BY folio DESC LIMIT 1")
            .fetch_optional(conn)
            .await?,
    )
}
