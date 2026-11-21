//! Exportar datos a Excel: productos, stock y valorización, ventas con su detalle, movimientos
//! de inventario, compras y clientes con su deuda. Se lee la base por páginas (cursor por fecha
//! e id, con los índices) y un hilo aparte escribe el .xlsx fila a fila: una exportación de cientos
//! de miles de filas no queda entera en memoria ni bloquea la caja.
//!
//! También las planillas de ejemplo para importar, con los mismos encabezados que reconoce la
//! importación (una exportación de productos o clientes se puede volver a importar).

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{FromRow, SqlitePool};
use tokio::sync::mpsc;

use crate::datos::excel::Escritor;
use crate::datos::{texto_rango, Columna, Valor};
use crate::repositorios::fiados::SALDO;
use crate::repositorios::productos::Unidad;
use crate::servicios::reportes::Rango;
use crate::servicios::ventas::MedioPago;
use crate::zona::{self, Zona};
use crate::{Error, Resultado};

const PAGINA: i64 = 5000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Exportacion {
    Productos,
    Stock,
    Ventas,
    Movimientos,
    Compras,
    Clientes,
}

impl Exportacion {
    /// Las que dependen de un rango de fechas.
    pub fn usa_rango(self) -> bool {
        matches!(
            self,
            Exportacion::Ventas | Exportacion::Movimientos | Exportacion::Compras
        )
    }

    pub fn nombre_base(self) -> &'static str {
        match self {
            Exportacion::Productos => "productos",
            Exportacion::Stock => "stock",
            Exportacion::Ventas => "ventas",
            Exportacion::Movimientos => "movimientos",
            Exportacion::Compras => "compras",
            Exportacion::Clientes => "clientes",
        }
    }

    fn titulo(self) -> &'static str {
        match self {
            Exportacion::Productos => "Productos",
            Exportacion::Stock => "Stock y valorización",
            Exportacion::Ventas => "Ventas",
            Exportacion::Movimientos => "Movimientos de inventario",
            Exportacion::Compras => "Compras",
            Exportacion::Clientes => "Clientes y deudas",
        }
    }
}

enum Mensaje {
    Hoja(String, Vec<Columna>),
    Fila(Vec<Valor>),
}

/// Manda filas al hilo que escribe. Si el escritor falló, deja de mandar (su error se informa).
struct Salida(mpsc::Sender<Mensaje>);

impl Salida {
    async fn hoja(&self, nombre: &str, columnas: Vec<Columna>) -> bool {
        self.0
            .send(Mensaje::Hoja(nombre.into(), columnas))
            .await
            .is_ok()
    }
    async fn fila(&self, fila: Vec<Valor>) -> bool {
        self.0.send(Mensaje::Fila(fila)).await.is_ok()
    }
}

fn unidad(u: &str) -> Unidad {
    if u == "kg" {
        Unidad::Kg
    } else {
        Unidad::Unidad
    }
}

fn cant(mil: i64, u: &str) -> Valor {
    Valor::Cantidad {
        mil,
        unidad: unidad(u),
    }
}

fn si_no(b: bool) -> Valor {
    Valor::texto(if b { "Sí" } else { "No" })
}

const ENCABEZADOS_PRODUCTOS: [(&str, u16); 8] = [
    ("Código de barras", 18),
    ("Nombre", 36),
    ("Categoría", 18),
    ("Unidad", 9),
    ("Precio", 12),
    ("Costo", 12),
    ("Stock", 11),
    ("Stock mínimo", 13),
];

const ENCABEZADOS_CLIENTES: [(&str, u16); 6] = [
    ("Nombre", 30),
    ("Teléfono", 16),
    ("RUT", 14),
    ("Dirección", 30),
    ("Nota", 24),
    ("Límite de crédito", 16),
];

fn columnas(lista: &[(&str, u16)]) -> Vec<Columna> {
    lista.iter().map(|(t, a)| Columna::new(t, *a)).collect()
}

/// Exporta a `ruta`. `rango` solo se usa en ventas, movimientos y compras.
pub async fn exportar(
    pool: &SqlitePool,
    zona: &'static dyn Zona,
    que: Exportacion,
    rango: Option<Rango>,
    ruta: PathBuf,
    ahora: DateTime<Utc>,
) -> Resultado<()> {
    let negocio = {
        let mut conn = pool.acquire().await?;
        crate::repositorios::negocio::obtener(&mut conn)
            .await?
            .nombre
    };
    let rango_texto = match (que.usa_rango(), rango) {
        (true, Some(r)) => texto_rango(r.desde, r.hasta),
        _ => String::new(),
    };
    let emitido = zona.a_local(ahora).format("%d-%m-%Y %H:%M").to_string();
    let titulo = que.titulo().to_owned();

    let (tx, mut rx) = mpsc::channel::<Mensaje>(2048);
    let escritor = tokio::task::spawn_blocking(move || -> Resultado<()> {
        let mut e = Escritor::new(&titulo, &negocio, &rango_texto, &emitido, zona);
        while let Some(m) = rx.blocking_recv() {
            match m {
                Mensaje::Hoja(n, c) => e.hoja(&n, &c)?,
                Mensaje::Fila(f) => e.fila(&f)?,
            }
        }
        e.guardar(&ruta)
    });
    let salida = Salida(tx);
    let limites = rango.map(|r| zona::limites(zona, r.desde, r.hasta));
    let leido = leer(pool, que, limites, &salida).await;
    drop(salida);
    let escrito = escritor
        .await
        .map_err(|e| Error::Validacion(format!("Se interrumpió la exportación: {e}")))?;
    // Si falló la escritura, ese es el error que importa (el lector solo dejó de mandar).
    escrito?;
    leido
}

async fn leer(
    pool: &SqlitePool,
    que: Exportacion,
    limites: Option<(String, String)>,
    s: &Salida,
) -> Resultado<()> {
    let mut conn = pool.acquire().await?;
    let (inicio, fin) = limites.unwrap_or_else(|| ("0000".into(), "9999".into()));
    match que {
        Exportacion::Productos => {
            #[derive(FromRow)]
            struct F {
                id: i64,
                codigo_barras: Option<String>,
                nombre: String,
                categoria: Option<String>,
                unidad: String,
                precio_venta: i64,
                costo: Option<i64>,
                stock_mil: i64,
                stock_minimo_mil: i64,
                activo: bool,
            }
            let mut cols = columnas(&ENCABEZADOS_PRODUCTOS);
            cols.push(Columna::new("Activo", 8));
            if !s.hoja("Productos", cols).await {
                return Ok(());
            }
            let mut ultimo = 0;
            loop {
                let filas: Vec<F> = sqlx::query_as(
                    "SELECT p.id, p.codigo_barras, p.nombre, c.nombre AS categoria, p.unidad,
                            p.precio_venta, p.costo, p.stock_mil, p.stock_minimo_mil, p.activo
                     FROM productos p LEFT JOIN categorias c ON c.id = p.categoria_id
                     WHERE p.id > ? ORDER BY p.id LIMIT ?",
                )
                .bind(ultimo)
                .bind(PAGINA)
                .fetch_all(&mut *conn)
                .await?;
                let Some(f) = filas.last() else { break };
                ultimo = f.id;
                for f in filas {
                    let fila = vec![
                        Valor::opcional_texto(f.codigo_barras),
                        Valor::Texto(f.nombre),
                        Valor::opcional_texto(f.categoria),
                        Valor::texto(f.unidad.clone()),
                        Valor::Pesos(f.precio_venta),
                        Valor::opcional_pesos(f.costo),
                        cant(f.stock_mil, &f.unidad),
                        cant(f.stock_minimo_mil, &f.unidad),
                        si_no(f.activo),
                    ];
                    if !s.fila(fila).await {
                        return Ok(());
                    }
                }
            }
        }
        Exportacion::Stock => {
            #[derive(FromRow)]
            struct F {
                id: i64,
                nombre: String,
                categoria: Option<String>,
                unidad: String,
                stock_mil: i64,
                stock_minimo_mil: i64,
                costo: Option<i64>,
                precio_venta: i64,
            }
            let cols = vec![
                Columna::new("Producto", 36),
                Columna::new("Categoría", 18),
                Columna::new("Unidad", 9),
                Columna::new("Stock", 11),
                Columna::new("Mínimo", 11),
                Columna::new("Costo", 12),
                Columna::new("Valor a costo", 14),
                Columna::new("Precio", 12),
                Columna::new("Valor a precio", 14),
                Columna::new("Estado", 14),
            ];
            if !s.hoja("Stock", cols).await {
                return Ok(());
            }
            // Stock × precio redondeado al peso; el stock negativo no vale (como en Inventario).
            let valor = |stock: i64, precio: i64| (stock.max(0) * precio + 500) / 1000;
            let mut ultimo = 0;
            loop {
                let filas: Vec<F> = sqlx::query_as(
                    "SELECT p.id, p.nombre, c.nombre AS categoria, p.unidad, p.stock_mil,
                            p.stock_minimo_mil, p.costo, p.precio_venta
                     FROM productos p LEFT JOIN categorias c ON c.id = p.categoria_id
                     WHERE p.activo = 1 AND p.id > ? ORDER BY p.id LIMIT ?",
                )
                .bind(ultimo)
                .bind(PAGINA)
                .fetch_all(&mut *conn)
                .await?;
                let Some(f) = filas.last() else { break };
                ultimo = f.id;
                for f in filas {
                    let estado = if f.stock_mil < 0 {
                        "Negativo"
                    } else if f.stock_mil == 0 {
                        "Sin stock"
                    } else if f.stock_minimo_mil > 0 && f.stock_mil <= f.stock_minimo_mil {
                        "Bajo el mínimo"
                    } else {
                        ""
                    };
                    let fila = vec![
                        Valor::Texto(f.nombre),
                        Valor::opcional_texto(f.categoria),
                        Valor::texto(f.unidad.clone()),
                        cant(f.stock_mil, &f.unidad),
                        cant(f.stock_minimo_mil, &f.unidad),
                        Valor::opcional_pesos(f.costo),
                        Valor::opcional_pesos(f.costo.map(|c| valor(f.stock_mil, c))),
                        Valor::Pesos(f.precio_venta),
                        Valor::Pesos(valor(f.stock_mil, f.precio_venta)),
                        Valor::texto(estado),
                    ];
                    if !s.fila(fila).await {
                        return Ok(());
                    }
                }
            }
        }
        Exportacion::Ventas => {
            #[derive(FromRow)]
            struct V {
                id: i64,
                folio: i64,
                fecha_hora: String,
                medio_pago: String,
                estado: String,
                cliente: Option<String>,
                subtotal: i64,
                descuento: i64,
                redondeo: i64,
                total: i64,
                monto_devuelto: i64,
            }
            let cols = vec![
                Columna::new("Folio", 9),
                Columna::new("Fecha", 17),
                Columna::new("Medio de pago", 14),
                Columna::new("Estado", 10),
                Columna::new("Cliente", 22),
                Columna::new("Subtotal", 12),
                Columna::new("Descuento", 11),
                Columna::new("Redondeo", 10),
                Columna::new("Total", 12),
                Columna::new("Devuelto", 12),
                Columna::new("Neto", 12),
            ];
            if !s.hoja("Ventas", cols).await {
                return Ok(());
            }
            let mut cursor = (inicio.clone(), 0i64);
            loop {
                let filas: Vec<V> = sqlx::query_as(
                    "SELECT v.id, v.folio, v.fecha_hora, v.medio_pago, v.estado, c.nombre AS cliente,
                            v.subtotal, v.descuento, v.redondeo, v.total, v.monto_devuelto
                     FROM ventas v LEFT JOIN clientes c ON c.id = v.cliente_id
                     WHERE (v.fecha_hora, v.id) > (?, ?) AND v.fecha_hora < ?
                     ORDER BY v.fecha_hora, v.id LIMIT ?",
                )
                .bind(&cursor.0)
                .bind(cursor.1)
                .bind(&fin)
                .bind(PAGINA)
                .fetch_all(&mut *conn)
                .await?;
                let Some(f) = filas.last() else { break };
                cursor = (f.fecha_hora.clone(), f.id);
                for v in filas {
                    let neto = if v.estado == "anulada" {
                        0
                    } else {
                        v.total - v.monto_devuelto
                    };
                    let fila = vec![
                        Valor::Entero(v.folio),
                        Valor::Fecha(v.fecha_hora),
                        Valor::texto(
                            MedioPago::desde_texto(&v.medio_pago).map_or("", MedioPago::nombre),
                        ),
                        Valor::Texto(v.estado),
                        Valor::opcional_texto(v.cliente),
                        Valor::Pesos(v.subtotal),
                        Valor::Pesos(v.descuento),
                        Valor::Pesos(v.redondeo),
                        Valor::Pesos(v.total),
                        Valor::Pesos(v.monto_devuelto),
                        Valor::Pesos(neto),
                    ];
                    if !s.fila(fila).await {
                        return Ok(());
                    }
                }
            }
            #[derive(FromRow)]
            struct L {
                id: i64,
                fecha_hora: String,
                folio: i64,
                nombre: String,
                unidad: String,
                cantidad_mil: i64,
                precio_unitario: i64,
                subtotal: i64,
                costo_unitario: Option<i64>,
                costo_estimado: bool,
            }
            let cols = vec![
                Columna::new("Folio", 9),
                Columna::new("Fecha", 17),
                Columna::new("Producto", 36),
                Columna::new("Cantidad", 11),
                Columna::new("Unidad", 9),
                Columna::new("Precio", 12),
                Columna::new("Subtotal", 12),
                Columna::new("Costo unitario", 14),
                Columna::new("Costo estimado", 14),
            ];
            if !s.hoja("Detalle", cols).await {
                return Ok(());
            }
            let mut cursor = (inicio, 0i64);
            loop {
                let filas: Vec<L> = sqlx::query_as(
                    "SELECT i.id, v.fecha_hora, v.folio, i.nombre, i.unidad, i.cantidad_mil,
                            i.precio_unitario, i.subtotal, i.costo_unitario, i.costo_estimado
                     FROM ventas v CROSS JOIN venta_items i ON i.venta_id = v.id
                     WHERE (v.fecha_hora, i.id) > (?, ?) AND v.fecha_hora < ?
                     ORDER BY v.fecha_hora, i.id LIMIT ?",
                )
                .bind(&cursor.0)
                .bind(cursor.1)
                .bind(&fin)
                .bind(PAGINA)
                .fetch_all(&mut *conn)
                .await?;
                let Some(f) = filas.last() else { break };
                cursor = (f.fecha_hora.clone(), f.id);
                for l in filas {
                    let fila = vec![
                        Valor::Entero(l.folio),
                        Valor::Fecha(l.fecha_hora),
                        Valor::Texto(l.nombre),
                        cant(l.cantidad_mil, &l.unidad),
                        Valor::texto(l.unidad.clone()),
                        Valor::Pesos(l.precio_unitario),
                        Valor::Pesos(l.subtotal),
                        Valor::opcional_pesos(l.costo_unitario),
                        if l.costo_unitario.is_some() {
                            si_no(l.costo_estimado)
                        } else {
                            Valor::Vacio
                        },
                    ];
                    if !s.fila(fila).await {
                        return Ok(());
                    }
                }
            }
        }
        Exportacion::Movimientos => {
            #[derive(FromRow)]
            struct M {
                id: i64,
                fecha: String,
                producto: String,
                unidad: String,
                tipo: String,
                cantidad_mil: i64,
                stock_resultante_mil: i64,
                motivo: Option<String>,
                nota: Option<String>,
                folio: Option<i64>,
                compra_id: Option<i64>,
            }
            let cols = vec![
                Columna::new("Fecha", 17),
                Columna::new("Producto", 36),
                Columna::new("Tipo", 11),
                Columna::new("Cantidad", 11),
                Columna::new("Stock resultante", 15),
                Columna::new("Motivo", 22),
                Columna::new("Nota", 22),
                Columna::new("Venta (folio)", 12),
                Columna::new("Compra N°", 10),
            ];
            if !s.hoja("Movimientos", cols).await {
                return Ok(());
            }
            let mut cursor = (inicio, 0i64);
            loop {
                let filas: Vec<M> = sqlx::query_as(
                    "SELECT m.id, m.fecha, p.nombre AS producto, p.unidad, m.tipo, m.cantidad_mil,
                            m.stock_resultante_mil, m.motivo, m.nota, v.folio, m.compra_id
                     FROM movimientos_stock m
                     JOIN productos p ON p.id = m.producto_id
                     LEFT JOIN ventas v ON v.id = m.venta_id
                     WHERE (m.fecha, m.id) > (?, ?) AND m.fecha < ?
                     ORDER BY m.fecha, m.id LIMIT ?",
                )
                .bind(&cursor.0)
                .bind(cursor.1)
                .bind(&fin)
                .bind(PAGINA)
                .fetch_all(&mut *conn)
                .await?;
                let Some(f) = filas.last() else { break };
                cursor = (f.fecha.clone(), f.id);
                for m in filas {
                    let fila = vec![
                        Valor::Fecha(m.fecha),
                        Valor::Texto(m.producto),
                        Valor::Texto(m.tipo),
                        cant(m.cantidad_mil, &m.unidad),
                        cant(m.stock_resultante_mil, &m.unidad),
                        Valor::opcional_texto(m.motivo),
                        Valor::opcional_texto(m.nota),
                        m.folio.map_or(Valor::Vacio, Valor::Entero),
                        m.compra_id.map_or(Valor::Vacio, Valor::Entero),
                    ];
                    if !s.fila(fila).await {
                        return Ok(());
                    }
                }
            }
        }
        Exportacion::Compras => {
            #[derive(FromRow)]
            struct C {
                id: i64,
                fecha: String,
                proveedor: Option<String>,
                documento: Option<String>,
                total: i64,
                pagada_con_caja: bool,
                anulada_en: Option<String>,
                motivo_anulacion: Option<String>,
            }
            let cols = vec![
                Columna::new("N°", 8),
                Columna::new("Fecha", 17),
                Columna::new("Proveedor", 26),
                Columna::new("Documento", 14),
                Columna::new("Total", 13),
                Columna::new("Pagada con la caja", 12),
                Columna::new("Anulada", 17),
                Columna::new("Motivo de anulación", 24),
            ];
            if !s.hoja("Compras", cols).await {
                return Ok(());
            }
            let mut cursor = (inicio.clone(), 0i64);
            loop {
                let filas: Vec<C> = sqlx::query_as(
                    "SELECT c.id, c.fecha, p.nombre AS proveedor, c.documento, c.total,
                            c.pagada_con_caja, c.anulada_en, c.motivo_anulacion
                     FROM compras c LEFT JOIN proveedores p ON p.id = c.proveedor_id
                     WHERE (c.fecha, c.id) > (?, ?) AND c.fecha < ?
                     ORDER BY c.fecha, c.id LIMIT ?",
                )
                .bind(&cursor.0)
                .bind(cursor.1)
                .bind(&fin)
                .bind(PAGINA)
                .fetch_all(&mut *conn)
                .await?;
                let Some(f) = filas.last() else { break };
                cursor = (f.fecha.clone(), f.id);
                for c in filas {
                    let fila = vec![
                        Valor::Entero(c.id),
                        Valor::Fecha(c.fecha),
                        Valor::opcional_texto(c.proveedor),
                        Valor::opcional_texto(c.documento),
                        Valor::Pesos(c.total),
                        si_no(c.pagada_con_caja),
                        c.anulada_en.map_or(Valor::Vacio, Valor::Fecha),
                        Valor::opcional_texto(c.motivo_anulacion),
                    ];
                    if !s.fila(fila).await {
                        return Ok(());
                    }
                }
            }
            #[derive(FromRow)]
            struct I {
                id: i64,
                fecha: String,
                compra_id: i64,
                producto: String,
                unidad: String,
                cantidad_mil: i64,
                costo_unitario: i64,
                subtotal: i64,
            }
            let cols = vec![
                Columna::new("Compra N°", 10),
                Columna::new("Fecha", 17),
                Columna::new("Producto", 36),
                Columna::new("Cantidad", 11),
                Columna::new("Costo unitario", 14),
                Columna::new("Subtotal", 13),
            ];
            if !s.hoja("Detalle", cols).await {
                return Ok(());
            }
            let mut cursor = (inicio, 0i64);
            loop {
                let filas: Vec<I> = sqlx::query_as(
                    "SELECT i.id, c.fecha, i.compra_id, p.nombre AS producto, p.unidad,
                            i.cantidad_mil, i.costo_unitario, i.subtotal
                     FROM compras c CROSS JOIN compra_items i ON i.compra_id = c.id
                     JOIN productos p ON p.id = i.producto_id
                     WHERE (c.fecha, i.id) > (?, ?) AND c.fecha < ?
                     ORDER BY c.fecha, i.id LIMIT ?",
                )
                .bind(&cursor.0)
                .bind(cursor.1)
                .bind(&fin)
                .bind(PAGINA)
                .fetch_all(&mut *conn)
                .await?;
                let Some(f) = filas.last() else { break };
                cursor = (f.fecha.clone(), f.id);
                for i in filas {
                    let fila = vec![
                        Valor::Entero(i.compra_id),
                        Valor::Fecha(i.fecha),
                        Valor::Texto(i.producto),
                        cant(i.cantidad_mil, &i.unidad),
                        Valor::Pesos(i.costo_unitario),
                        Valor::Pesos(i.subtotal),
                    ];
                    if !s.fila(fila).await {
                        return Ok(());
                    }
                }
            }
        }
        Exportacion::Clientes => {
            #[derive(FromRow)]
            struct K {
                id: i64,
                nombre: String,
                telefono: Option<String>,
                rut: Option<String>,
                direccion: Option<String>,
                nota: Option<String>,
                limite_credito: Option<i64>,
                activo: bool,
                deuda: i64,
                desde: Option<String>,
            }
            let mut cols = columnas(&ENCABEZADOS_CLIENTES);
            cols.extend([
                Columna::new("Deuda", 13),
                Columna::new("Debe desde", 17),
                Columna::new("Activo", 8),
            ]);
            if !s.hoja("Clientes", cols).await {
                return Ok(());
            }
            let mut ultimo = 0;
            loop {
                let sql = format!(
                    "SELECT c.id, c.nombre, c.telefono, c.rut, c.direccion, c.nota,
                            c.limite_credito, c.activo,
                            COALESCE((SELECT SUM({SALDO}) FROM ventas v
                                      WHERE v.cliente_id = c.id AND v.estado = 'fiada'), 0) AS deuda,
                            (SELECT MIN(v.fecha_hora) FROM ventas v
                             WHERE v.cliente_id = c.id AND v.estado = 'fiada') AS desde
                     FROM clientes c WHERE c.id > ? ORDER BY c.id LIMIT ?"
                );
                let filas: Vec<K> = sqlx::query_as(&sql)
                    .bind(ultimo)
                    .bind(PAGINA)
                    .fetch_all(&mut *conn)
                    .await?;
                let Some(f) = filas.last() else { break };
                ultimo = f.id;
                for k in filas {
                    let fila = vec![
                        Valor::Texto(k.nombre),
                        Valor::opcional_texto(k.telefono),
                        Valor::opcional_texto(k.rut),
                        Valor::opcional_texto(k.direccion),
                        Valor::opcional_texto(k.nota),
                        Valor::opcional_pesos(k.limite_credito),
                        Valor::Pesos(k.deuda),
                        k.desde.map_or(Valor::Vacio, Valor::Fecha),
                        si_no(k.activo),
                    ];
                    if !s.fila(fila).await {
                        return Ok(());
                    }
                }
            }
        }
    }
    Ok(())
}

/// La planilla de ejemplo para importar: solo los encabezados y dos filas de muestra. Es trabajo
/// de disco: fuera de los hilos async.
pub fn planilla_ejemplo(
    tipo: crate::servicios::importacion::Tipo,
    ruta: &std::path::Path,
) -> Resultado<()> {
    use crate::servicios::importacion::Tipo;
    let mut e = Escritor::new("", "", "", "", &zona::ZonaLocal);
    match tipo {
        Tipo::Productos => {
            e.hoja("Productos", &columnas(&ENCABEZADOS_PRODUCTOS))?;
            e.fila(&[
                Valor::texto("7801610001196"),
                Valor::texto("Leche entera 1 L"),
                Valor::texto("Lácteos"),
                Valor::texto("unidad"),
                Valor::Pesos(1190),
                Valor::Pesos(850),
                cant(24_000, "unidad"),
                cant(6_000, "unidad"),
            ])?;
            e.fila(&[
                Valor::texto("2002"),
                Valor::texto("Queso mantecoso"),
                Valor::texto("Lácteos"),
                Valor::texto("kg"),
                Valor::Pesos(10_490),
                Valor::Pesos(7_200),
                cant(2_500, "kg"),
                cant(1_000, "kg"),
            ])?;
        }
        Tipo::Clientes => {
            e.hoja("Clientes", &columnas(&ENCABEZADOS_CLIENTES))?;
            e.fila(&[
                Valor::texto("María González"),
                Valor::texto("+56 9 8765 4321"),
                Valor::texto("11.111.111-1"),
                Valor::texto("Los Aromos 123"),
                Valor::Vacio,
                Valor::Pesos(30_000),
            ])?;
            e.fila(&[
                Valor::texto("Juan Pérez"),
                Valor::texto("+56 9 5555 1234"),
                Valor::Vacio,
                Valor::Vacio,
                Valor::texto("Paga los viernes"),
                Valor::Vacio,
            ])?;
        }
    }
    e.guardar(ruta)
}
