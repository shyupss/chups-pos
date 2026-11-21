use sqlx::{FromRow, QueryBuilder, Sqlite, SqliteConnection, SqlitePool};

use crate::repositorios::productos::Unidad;
use crate::Resultado;

// --- Historial ------------------------------------------------------------------------------

/// Estado que se busca en el historial. "Con devolución" incluye las devueltas por completo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoBuscado {
    Pagada,
    Fiada,
    Anulada,
    ConDevolucion,
}

/// Filtro del historial ya listo para SQL: fechas en UTC (`hasta` excluida).
#[derive(Debug, Clone, Default)]
pub struct Filtro {
    pub desde_utc: Option<String>,
    pub hasta_utc: Option<String>,
    pub folio: Option<i64>,
    pub medio_pago: Option<&'static str>,
    pub estado: Option<EstadoBuscado>,
    pub turno_id: Option<i64>,
}

/// Agrega el WHERE del filtro. El folio exacto manda: con folio no se miran los demás filtros.
fn condiciones(q: &mut QueryBuilder<'_, Sqlite>, f: &Filtro) {
    q.push(" WHERE 1 = 1");
    if let Some(folio) = f.folio {
        q.push(" AND v.folio = ").push_bind(folio);
        return;
    }
    if let Some(desde) = &f.desde_utc {
        q.push(" AND v.fecha_hora >= ").push_bind(desde.clone());
    }
    if let Some(hasta) = &f.hasta_utc {
        q.push(" AND v.fecha_hora < ").push_bind(hasta.clone());
    }
    if let Some(medio) = f.medio_pago {
        q.push(" AND v.medio_pago = ").push_bind(medio);
    }
    if let Some(turno) = f.turno_id {
        q.push(" AND v.turno_id = ").push_bind(turno);
    }
    match f.estado {
        None => {}
        Some(EstadoBuscado::Pagada) => {
            q.push(" AND v.estado = 'pagada' AND v.devolucion IS NULL");
        }
        Some(EstadoBuscado::Fiada) => {
            q.push(" AND v.estado = 'fiada' AND v.devolucion IS NULL");
        }
        Some(EstadoBuscado::Anulada) => {
            q.push(" AND v.estado = 'anulada'");
        }
        Some(EstadoBuscado::ConDevolucion) => {
            q.push(" AND v.devolucion IS NOT NULL");
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct FilaListada {
    pub id: i64,
    pub folio: i64,
    pub fecha_hora: String,
    /// Artículos: las unidades una a una y cada pesada como uno (igual que la boleta).
    pub productos: i64,
    pub total: i64,
    pub medio_pago: String,
    pub estado: String,
    pub devolucion: Option<String>,
}

/// Una página del historial, de la más nueva a la más antigua (por el índice de fecha o turno).
pub async fn listar(
    pool: &SqlitePool,
    f: &Filtro,
    limite: i64,
    desplazamiento: i64,
) -> Resultado<Vec<FilaListada>> {
    let mut q = QueryBuilder::new(
        "SELECT v.id, v.folio, v.fecha_hora, v.total, v.medio_pago, v.estado, v.devolucion,
                (SELECT COALESCE(SUM(CASE WHEN i.unidad = 'unidad' AND i.cantidad_mil % 1000 = 0
                                          THEN i.cantidad_mil / 1000 ELSE 1 END), 0)
                 FROM venta_items i WHERE i.venta_id = v.id) AS productos
         FROM ventas v",
    );
    condiciones(&mut q, f);
    q.push(" ORDER BY v.fecha_hora DESC, v.id DESC LIMIT ")
        .push_bind(limite)
        .push(" OFFSET ")
        .push_bind(desplazamiento);
    Ok(q.build_query_as().fetch_all(pool).await?)
}

/// Cantidad de ventas del filtro y lo vendido: sin las anuladas y descontando lo devuelto.
pub async fn totales(pool: &SqlitePool, f: &Filtro) -> Resultado<(i64, i64)> {
    let mut q = QueryBuilder::new(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN v.estado = 'anulada' THEN 0
                                  ELSE v.total - v.monto_devuelto END), 0)
         FROM ventas v",
    );
    condiciones(&mut q, f);
    Ok(q.build_query_as().fetch_one(pool).await?)
}

pub async fn id_por_folio(pool: &SqlitePool, folio: i64) -> Resultado<Option<i64>> {
    Ok(sqlx::query_scalar("SELECT id FROM ventas WHERE folio = ?")
        .bind(folio)
        .fetch_optional(pool)
        .await?)
}

/// Todo lo de la cabecera para el detalle del historial.
#[derive(Debug, Clone, FromRow)]
pub struct FilaDetalle {
    pub id: i64,
    pub folio: i64,
    pub fecha_hora: String,
    pub subtotal: i64,
    pub descuento: i64,
    pub redondeo: i64,
    pub total: i64,
    pub medio_pago: String,
    pub monto_recibido: Option<i64>,
    pub vuelto: Option<i64>,
    pub estado: String,
    pub cliente: Option<String>,
    pub turno_id: Option<i64>,
    pub anulada_en: Option<String>,
    pub motivo_anulacion: Option<String>,
    pub turno_anulacion_id: Option<i64>,
    pub medio_reembolso_anulacion: Option<String>,
    pub reembolso_anulacion: Option<i64>,
    pub monto_devuelto: i64,
    pub devolucion: Option<String>,
    pub abonos: i64,
}

pub async fn detalle(conn: &mut SqliteConnection, id: i64) -> Resultado<Option<FilaDetalle>> {
    Ok(sqlx::query_as(
        "SELECT v.id, v.folio, v.fecha_hora, v.subtotal, v.descuento, v.redondeo, v.total,
                v.medio_pago, v.monto_recibido, v.vuelto, v.estado, c.nombre AS cliente,
                v.turno_id, v.anulada_en, v.motivo_anulacion, v.turno_anulacion_id,
                v.medio_reembolso_anulacion, v.reembolso_anulacion, v.monto_devuelto,
                v.devolucion,
                (SELECT COUNT(*) FROM abonos a WHERE a.venta_id = v.id) AS abonos
         FROM ventas v LEFT JOIN clientes c ON c.id = v.cliente_id
         WHERE v.id = ?",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?)
}

/// Línea vendida con su id y lo que ya se devolvió de ella.
#[derive(Debug, Clone, FromRow)]
pub struct FilaItem {
    pub id: i64,
    pub producto_id: i64,
    pub nombre: String,
    pub unidad: Unidad,
    pub precio_unitario: i64,
    pub cantidad_mil: i64,
    pub subtotal: i64,
    pub devuelto_mil: i64,
    /// Lo que ya se devolvió de su subtotal (antes del redondeo del efectivo).
    pub devuelto_monto: i64,
}

pub async fn items_con_devuelto(
    conn: &mut SqliteConnection,
    venta_id: i64,
) -> Resultado<Vec<FilaItem>> {
    Ok(sqlx::query_as(
        "SELECT i.id, i.producto_id, i.nombre, i.unidad, i.precio_unitario, i.cantidad_mil,
                i.subtotal,
                COALESCE((SELECT SUM(d.cantidad_mil) FROM devolucion_items d
                          WHERE d.venta_item_id = i.id), 0) AS devuelto_mil,
                COALESCE((SELECT SUM(d.monto) FROM devolucion_items d
                          WHERE d.venta_item_id = i.id), 0) AS devuelto_monto
         FROM venta_items i WHERE i.venta_id = ? ORDER BY i.id",
    )
    .bind(venta_id)
    .fetch_all(conn)
    .await?)
}

pub struct NuevaCabecera<'a> {
    /// Turno abierto en que se cobra.
    pub turno_id: i64,
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
            (folio, turno_id, subtotal, descuento, redondeo, total, medio_pago, monto_recibido,
             vuelto, estado, cliente_id)
         VALUES ((SELECT COALESCE(MAX(folio), 0) + 1 FROM ventas), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         RETURNING id, folio, fecha_hora",
    )
    .bind(v.turno_id)
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
    pub anulada_en: Option<String>,
}

pub async fn cabecera(
    conn: &mut SqliteConnection,
    venta_id: i64,
) -> Resultado<Option<CabeceraVenta>> {
    Ok(sqlx::query_as(
        "SELECT v.folio, v.fecha_hora, v.subtotal, v.descuento, v.redondeo, v.total, v.medio_pago,
                v.monto_recibido, v.vuelto, c.nombre AS cliente, v.anulada_en
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

pub struct DatosAnulacion<'a> {
    pub motivo: &'a str,
    pub turno_id: Option<i64>,
    /// `None` en una venta fiada (no se devuelve dinero).
    pub medio_reembolso: Option<&'a str>,
    pub reembolso: Option<i64>,
}

/// Marca la venta como anulada si sigue pagada o fiada y sin devoluciones.
/// Devuelve `false` si ya no lo estaba (otra anulación o devolución llegó antes).
pub async fn anular(
    conn: &mut SqliteConnection,
    venta_id: i64,
    a: &DatosAnulacion<'_>,
) -> Resultado<bool> {
    let r = sqlx::query(
        "UPDATE ventas
         SET estado = 'anulada', anulada_en = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             motivo_anulacion = ?, turno_anulacion_id = ?, medio_reembolso_anulacion = ?,
             reembolso_anulacion = ?
         WHERE id = ? AND estado IN ('pagada', 'fiada') AND devolucion IS NULL",
    )
    .bind(a.motivo)
    .bind(a.turno_id)
    .bind(a.medio_reembolso)
    .bind(a.reembolso)
    .bind(venta_id)
    .execute(conn)
    .await?;
    Ok(r.rows_affected() == 1)
}
