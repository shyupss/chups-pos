use sqlx::SqliteConnection;

use crate::Resultado;

pub struct NuevaCabecera<'a> {
    pub subtotal: i64,
    pub descuento: i64,
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
            (folio, subtotal, descuento, total, medio_pago, monto_recibido, vuelto, estado, cliente_id)
         VALUES ((SELECT COALESCE(MAX(folio), 0) + 1 FROM ventas), ?, ?, ?, ?, ?, ?, ?, ?)
         RETURNING id, folio, fecha_hora",
    )
    .bind(v.subtotal)
    .bind(v.descuento)
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

pub async fn insertar_item(
    conn: &mut SqliteConnection,
    venta_id: i64,
    producto_id: i64,
    nombre: &str,
    precio_unitario: i64,
    cantidad_mil: i64,
    subtotal: i64,
) -> Resultado<()> {
    sqlx::query(
        "INSERT INTO venta_items (venta_id, producto_id, nombre, precio_unitario, cantidad_mil, subtotal)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(venta_id)
    .bind(producto_id)
    .bind(nombre)
    .bind(precio_unitario)
    .bind(cantidad_mil)
    .bind(subtotal)
    .execute(conn)
    .await?;
    Ok(())
}
