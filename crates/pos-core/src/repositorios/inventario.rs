use sqlx::{FromRow, QueryBuilder, Sqlite, SqlitePool};

use crate::repositorios::productos::{FiltroCategoria, Unidad};
use crate::Resultado;

/// Qué productos mostrar en la pestaña Stock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EstadoStock {
    #[default]
    Todos,
    /// Con mínimo definido y stock igual o menor.
    BajoMinimo,
    /// Stock exactamente cero.
    SinStock,
    Negativo,
}

#[derive(Debug, Clone, Default)]
pub struct FiltroStock<'a> {
    /// Consulta FTS ya armada (nombre) y prefijo de código, como en Productos.
    pub fts: Option<&'a str>,
    pub prefijo_codigo: Option<&'a str>,
    pub categoria: FiltroCategoria,
    pub estado: EstadoStock,
}

#[derive(Debug, Clone, FromRow)]
pub struct FilaStock {
    pub id: i64,
    pub codigo_barras: Option<String>,
    pub nombre: String,
    pub categoria_id: Option<i64>,
    pub unidad: Unidad,
    pub precio_venta: i64,
    pub costo: Option<i64>,
    pub stock_mil: i64,
    pub stock_minimo_mil: i64,
}

/// Productos activos con su stock, por nombre. Paginado: nunca el catálogo completo.
pub async fn listar_stock(
    pool: &SqlitePool,
    f: &FiltroStock<'_>,
    limite: i64,
    desplazamiento: i64,
) -> Resultado<Vec<FilaStock>> {
    let mut q = QueryBuilder::<Sqlite>::new(
        "SELECT p.id, p.codigo_barras, p.nombre, p.categoria_id, p.unidad, p.precio_venta,
                p.costo, p.stock_mil, p.stock_minimo_mil
         FROM productos p WHERE p.activo = 1",
    );
    match (f.fts, f.prefijo_codigo) {
        (Some(fts), Some(prefijo)) => {
            q.push(" AND (p.id IN (SELECT rowid FROM productos_fts WHERE productos_fts MATCH ")
                .push_bind(fts.to_owned())
                .push(") OR p.codigo_barras LIKE ")
                .push_bind(format!("{prefijo}%"))
                .push(")");
        }
        (Some(fts), None) => {
            q.push(" AND p.id IN (SELECT rowid FROM productos_fts WHERE productos_fts MATCH ")
                .push_bind(fts.to_owned())
                .push(")");
        }
        (None, Some(prefijo)) => {
            q.push(" AND p.codigo_barras LIKE ")
                .push_bind(format!("{prefijo}%"));
        }
        (None, None) => {}
    }
    match f.categoria {
        FiltroCategoria::Todas => {}
        FiltroCategoria::SinCategoria => {
            q.push(" AND p.categoria_id IS NULL");
        }
        FiltroCategoria::Una(id) => {
            q.push(" AND p.categoria_id = ").push_bind(id);
        }
    }
    match f.estado {
        EstadoStock::Todos => {}
        EstadoStock::BajoMinimo => {
            q.push(" AND p.stock_minimo_mil > 0 AND p.stock_mil - p.stock_minimo_mil <= 0");
        }
        EstadoStock::SinStock => {
            q.push(" AND p.stock_mil = 0");
        }
        EstadoStock::Negativo => {
            q.push(" AND p.stock_mil < 0");
        }
    }
    q.push(" ORDER BY p.nombre COLLATE NOCASE, p.id LIMIT ")
        .push_bind(limite)
        .push(" OFFSET ")
        .push_bind(desplazamiento);
    Ok(q.build_query_as().fetch_all(pool).await?)
}

/// Totales del inventario (productos activos; el stock negativo no suma).
#[derive(Debug, Clone, Copy, FromRow, PartialEq, Eq)]
pub struct Valorizacion {
    /// Stock × costo, redondeado al peso por producto. Los productos sin costo cuentan como 0.
    pub valor_costo: i64,
    pub valor_venta: i64,
    /// Productos con stock que no tienen costo (su valor a costo no se conoce).
    pub sin_costo: i64,
    pub bajo_minimo: i64,
    pub productos: i64,
}

pub async fn valorizacion(pool: &SqlitePool) -> Resultado<Valorizacion> {
    Ok(sqlx::query_as(
        "SELECT
            COALESCE(SUM(CASE WHEN stock_mil > 0 AND costo IS NOT NULL
                              THEN (stock_mil * costo + 500) / 1000 ELSE 0 END), 0) AS valor_costo,
            COALESCE(SUM(CASE WHEN stock_mil > 0
                              THEN (stock_mil * precio_venta + 500) / 1000 ELSE 0 END), 0)
                AS valor_venta,
            COALESCE(SUM(stock_mil > 0 AND costo IS NULL), 0) AS sin_costo,
            COALESCE(SUM(stock_minimo_mil > 0 AND stock_mil - stock_minimo_mil <= 0), 0)
                AS bajo_minimo,
            COUNT(*) AS productos
         FROM productos WHERE activo = 1",
    )
    .fetch_one(pool)
    .await?)
}

/// Productos activos bajo el mínimo, por el índice parcial idx_productos_bajo_minimo.
pub async fn contar_bajo_minimo(pool: &SqlitePool) -> Resultado<i64> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM productos
         WHERE activo = 1 AND stock_minimo_mil > 0 AND stock_mil - stock_minimo_mil <= 0",
    )
    .fetch_one(pool)
    .await?)
}

#[derive(Debug, Clone, Default)]
pub struct FiltroMovimientos {
    pub producto_id: Option<i64>,
    pub tipo: Option<&'static str>,
    pub desde_utc: Option<String>,
    pub hasta_utc: Option<String>,
    /// Cursor: la página sigue después de (fecha, id) del último movimiento ya mostrado.
    pub despues_de: Option<(String, i64)>,
}

#[derive(Debug, Clone, FromRow)]
pub struct FilaMovimiento {
    pub id: i64,
    pub fecha: String,
    pub producto_id: i64,
    pub producto: String,
    pub unidad: Unidad,
    pub tipo: String,
    pub cantidad_mil: i64,
    pub stock_resultante_mil: i64,
    pub motivo: Option<String>,
    pub nota: Option<String>,
    pub venta_id: Option<i64>,
    pub folio: Option<i64>,
    pub compra_id: Option<i64>,
}

/// Movimientos del más nuevo al más antiguo. Pagina con un cursor (fecha, id) para que la página
/// 500 cueste lo mismo que la primera: usa idx_movimientos_fecha, _tipo_fecha o _producto_fecha.
pub async fn listar_movimientos(
    pool: &SqlitePool,
    f: &FiltroMovimientos,
    limite: i64,
) -> Resultado<Vec<FilaMovimiento>> {
    let mut q = QueryBuilder::<Sqlite>::new(
        "SELECT m.id, m.fecha, m.producto_id, p.nombre AS producto, p.unidad, m.tipo,
                m.cantidad_mil, m.stock_resultante_mil, m.motivo, m.nota, m.venta_id,
                v.folio, m.compra_id
         FROM movimientos_stock m
         JOIN productos p ON p.id = m.producto_id
         LEFT JOIN ventas v ON v.id = m.venta_id
         WHERE 1 = 1",
    );
    if let Some(id) = f.producto_id {
        q.push(" AND m.producto_id = ").push_bind(id);
    }
    if let Some(tipo) = f.tipo {
        q.push(" AND m.tipo = ").push_bind(tipo);
    }
    if let Some(desde) = &f.desde_utc {
        q.push(" AND m.fecha >= ").push_bind(desde.clone());
    }
    if let Some(hasta) = &f.hasta_utc {
        q.push(" AND m.fecha < ").push_bind(hasta.clone());
    }
    if let Some((fecha, id)) = &f.despues_de {
        q.push(" AND (m.fecha, m.id) < (")
            .push_bind(fecha.clone())
            .push(", ")
            .push_bind(*id)
            .push(")");
    }
    q.push(" ORDER BY m.fecha DESC, m.id DESC LIMIT ")
        .push_bind(limite);
    Ok(q.build_query_as().fetch_all(pool).await?)
}
