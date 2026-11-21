//! Inventario: el stock con su valor, el historial de movimientos, los ajustes por conteo y las
//! mermas. Todo cambio de stock queda como movimiento: nada se edita a mano.
//!
//! - Ajuste por conteo: el usuario dice cuánto contó y Rust calcula la diferencia contra el stock
//!   del momento (movimiento 'ajuste'). Varios productos se confirman en una sola transacción.
//! - Merma: lo que se pierde (vencido, dañado, robo, consumo interno u otro), movimiento 'merma'.
//! - Ambos piden el PIN si "ajustar inventario" está protegida.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::validacion;
use crate::repositorios::inventario::{
    self as repo, EstadoStock, FiltroMovimientos, FiltroStock, Valorizacion,
};
use crate::repositorios::productos::{self, FiltroCategoria, Movimiento, Producto, Unidad};
use crate::servicios::cantidades::{self, MIL};
use crate::servicios::historial;
use crate::servicios::productos::{consulta_fts, parece_codigo};
use crate::servicios::seguridad::{self, Accion, ControlPin};
use crate::{Error, Resultado};

/// Filtro de la pestaña Stock. `categoria_id` y `sin_categoria` como en Productos.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiltrosStock {
    #[serde(default)]
    pub texto: String,
    pub categoria_id: Option<i64>,
    #[serde(default)]
    pub sin_categoria: bool,
    #[serde(default)]
    pub estado: FiltroEstadoStock,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FiltroEstadoStock {
    #[default]
    Todos,
    BajoMinimo,
    SinStock,
    Negativo,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProductoStock {
    pub id: i64,
    pub codigo_barras: Option<String>,
    pub nombre: String,
    pub categoria_id: Option<i64>,
    pub unidad: Unidad,
    pub precio_venta: i64,
    pub costo: Option<i64>,
    pub stock_mil: i64,
    pub stock_minimo_mil: i64,
    /// Stock × costo redondeado al peso; `None` si no tiene costo. El stock negativo vale 0.
    pub valor: Option<i64>,
    pub bajo_minimo: bool,
}

/// Valor de un stock en milésimas a un precio por unidad o por kg.
fn valor(stock_mil: i64, precio: i64) -> i64 {
    cantidades::subtotal(precio, stock_mil.max(0)).unwrap_or(i64::MAX)
}

pub async fn listar_stock(
    pool: &SqlitePool,
    f: &FiltrosStock,
    limite: i64,
    desplazamiento: i64,
) -> Resultado<Vec<ProductoStock>> {
    let texto = f.texto.trim();
    let fts = consulta_fts(texto);
    let prefijo = parece_codigo(texto).then_some(texto);
    if !texto.is_empty() && fts.is_none() && prefijo.is_none() {
        return Ok(Vec::new());
    }
    let filtro = FiltroStock {
        fts: fts.as_deref(),
        prefijo_codigo: prefijo,
        categoria: match (f.categoria_id, f.sin_categoria) {
            (_, true) => FiltroCategoria::SinCategoria,
            (Some(id), false) => FiltroCategoria::Una(id),
            (None, false) => FiltroCategoria::Todas,
        },
        estado: match f.estado {
            FiltroEstadoStock::Todos => EstadoStock::Todos,
            FiltroEstadoStock::BajoMinimo => EstadoStock::BajoMinimo,
            FiltroEstadoStock::SinStock => EstadoStock::SinStock,
            FiltroEstadoStock::Negativo => EstadoStock::Negativo,
        },
    };
    let filas = repo::listar_stock(
        pool,
        &filtro,
        limite.clamp(1, crate::servicios::productos::LIMITE_MAXIMO),
        desplazamiento.max(0),
    )
    .await?;
    Ok(filas
        .into_iter()
        .map(|p| ProductoStock {
            valor: p.costo.map(|c| valor(p.stock_mil, c)),
            bajo_minimo: p.stock_minimo_mil > 0 && p.stock_mil <= p.stock_minimo_mil,
            id: p.id,
            codigo_barras: p.codigo_barras,
            nombre: p.nombre,
            categoria_id: p.categoria_id,
            unidad: p.unidad,
            precio_venta: p.precio_venta,
            costo: p.costo,
            stock_mil: p.stock_mil,
            stock_minimo_mil: p.stock_minimo_mil,
        })
        .collect())
}

/// Valor del inventario a costo y a precio de venta, productos sin costo y bajo el mínimo.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResumenInventario {
    pub valor_costo: i64,
    pub valor_venta: i64,
    pub sin_costo: i64,
    pub bajo_minimo: i64,
    pub productos: i64,
}

pub async fn resumen(pool: &SqlitePool) -> Resultado<ResumenInventario> {
    let Valorizacion {
        valor_costo,
        valor_venta,
        sin_costo,
        bajo_minimo,
        productos,
    } = repo::valorizacion(pool).await?;
    Ok(ResumenInventario {
        valor_costo,
        valor_venta,
        sin_costo,
        bajo_minimo,
        productos,
    })
}

/// La insignia de la barra lateral: una consulta por índice.
pub async fn contar_bajo_minimo(pool: &SqlitePool) -> Resultado<i64> {
    repo::contar_bajo_minimo(pool).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoMovimientoStock {
    Venta,
    Compra,
    Ajuste,
    Merma,
    Devolucion,
}

impl TipoMovimientoStock {
    pub fn como_texto(self) -> &'static str {
        match self {
            Self::Venta => "venta",
            Self::Compra => "compra",
            Self::Ajuste => "ajuste",
            Self::Merma => "merma",
            Self::Devolucion => "devolucion",
        }
    }

    fn desde_texto(t: &str) -> Self {
        match t {
            "venta" => Self::Venta,
            "compra" => Self::Compra,
            "merma" => Self::Merma,
            "devolucion" => Self::Devolucion,
            _ => Self::Ajuste,
        }
    }
}

/// Filtro de la pestaña Movimientos. Fechas como días locales "aaaa-mm-dd" (hasta incluido).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiltrosMovimientos {
    pub producto_id: Option<i64>,
    pub tipo: Option<TipoMovimientoStock>,
    pub desde: Option<String>,
    pub hasta: Option<String>,
}

/// Dónde continuar: la fecha y el id del último movimiento de la página anterior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CursorMovimientos {
    pub fecha: String,
    pub id: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MovimientoStock {
    pub id: i64,
    pub fecha: String,
    pub producto_id: i64,
    pub producto: String,
    pub unidad: Unidad,
    pub tipo: TipoMovimientoStock,
    /// Positivo entra, negativo sale (en milésimas).
    pub cantidad_mil: i64,
    pub stock_resultante_mil: i64,
    pub motivo: Option<String>,
    pub nota: Option<String>,
    /// Venta de origen (id y folio) o compra de origen.
    pub venta_id: Option<i64>,
    pub folio: Option<i64>,
    pub compra_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PaginaMovimientos {
    pub movimientos: Vec<MovimientoStock>,
    /// `None` si no hay más.
    pub siguiente: Option<CursorMovimientos>,
}

pub async fn listar_movimientos(
    pool: &SqlitePool,
    f: &FiltrosMovimientos,
    despues_de: Option<CursorMovimientos>,
    limite: i64,
) -> Resultado<PaginaMovimientos> {
    let desde = f.desde.as_deref().map(historial::dia).transpose()?;
    let hasta = f.hasta.as_deref().map(historial::dia).transpose()?;
    if let (Some(d), Some(h)) = (desde, hasta) {
        if d > h {
            return validacion("La fecha de inicio es posterior a la de término");
        }
    }
    let limite = limite.clamp(1, 500);
    let filas = repo::listar_movimientos(
        pool,
        &FiltroMovimientos {
            producto_id: f.producto_id,
            tipo: f.tipo.map(TipoMovimientoStock::como_texto),
            desde_utc: desde.map(historial::inicio_del_dia),
            hasta_utc: hasta
                .and_then(|h| h.succ_opt())
                .map(historial::inicio_del_dia),
            despues_de: despues_de.map(|c| (c.fecha, c.id)),
        },
        limite,
    )
    .await?;
    let siguiente = (filas.len() as i64 == limite).then(|| {
        let ultima = filas.last().expect("página llena");
        CursorMovimientos {
            fecha: ultima.fecha.clone(),
            id: ultima.id,
        }
    });
    Ok(PaginaMovimientos {
        movimientos: filas
            .into_iter()
            .map(|m| MovimientoStock {
                tipo: TipoMovimientoStock::desde_texto(&m.tipo),
                id: m.id,
                fecha: m.fecha,
                producto_id: m.producto_id,
                producto: m.producto,
                unidad: m.unidad,
                cantidad_mil: m.cantidad_mil,
                stock_resultante_mil: m.stock_resultante_mil,
                motivo: m.motivo,
                nota: m.nota,
                venta_id: m.venta_id,
                folio: m.folio,
                compra_id: m.compra_id,
            })
            .collect(),
        siguiente,
    })
}

/// Lo que se contó de un producto, en milésimas.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contado {
    pub producto_id: i64,
    pub contado_mil: i64,
}

/// Diferencia de un producto contado contra su stock del momento.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Diferencia {
    pub producto_id: i64,
    pub nombre: String,
    pub unidad: Unidad,
    pub stock_mil: i64,
    pub contado_mil: i64,
    /// contado − stock: positivo sobra, negativo falta.
    pub diferencia_mil: i64,
    /// La diferencia valorizada a costo; `None` si el producto no tiene costo.
    pub valor: Option<i64>,
}

fn diferencia(p: &Producto, contado_mil: i64) -> Diferencia {
    let diferencia_mil = contado_mil - p.stock_mil;
    Diferencia {
        producto_id: p.id,
        nombre: p.nombre.clone(),
        unidad: p.unidad,
        stock_mil: p.stock_mil,
        contado_mil,
        diferencia_mil,
        valor: p.costo.map(|c| {
            let v = cantidades::subtotal(c, diferencia_mil.abs()).unwrap_or(i64::MAX);
            if diferencia_mil < 0 {
                -v
            } else {
                v
            }
        }),
    }
}

/// Un stock contado: no negativo, por unidad en unidades enteras y con el mismo tope que una venta.
fn validar_contado(p: &Producto, contado_mil: i64) -> Resultado<()> {
    if contado_mil < 0 {
        return validacion(format!("Lo contado de {} no puede ser negativo", p.nombre));
    }
    if contado_mil == 0 {
        return Ok(());
    }
    match p.unidad {
        Unidad::Unidad if contado_mil % MIL != 0 => validacion(format!(
            "{} se cuenta por unidad: escribe un número entero",
            p.nombre
        )),
        Unidad::Unidad if contado_mil > cantidades::MAX_UNIDADES_MIL => {
            validacion(format!("Lo contado de {} es demasiado", p.nombre))
        }
        Unidad::Kg if contado_mil > 10 * cantidades::MAX_KG_MIL => {
            validacion(format!("Lo contado de {} pasa de 1.000 kg", p.nombre))
        }
        _ => Ok(()),
    }
}

/// Revisa la lista: no vacía y cada producto una sola vez.
fn validar_lista(items: &[Contado]) -> Resultado<()> {
    if items.is_empty() {
        return validacion("Cuenta al menos un producto");
    }
    let mut vistos = HashSet::new();
    if items.iter().any(|i| !vistos.insert(i.producto_id)) {
        return validacion("Cada producto va una sola vez en el conteo");
    }
    Ok(())
}

/// Las diferencias que dejaría el conteo, sin guardar nada (el resumen antes de confirmar).
pub async fn previsualizar_conteo(
    pool: &SqlitePool,
    items: &[Contado],
) -> Resultado<Vec<Diferencia>> {
    validar_lista(items)?;
    let mut conn = pool.acquire().await?;
    let mut salida = Vec::with_capacity(items.len());
    for i in items {
        let p = productos::por_id(&mut conn, i.producto_id)
            .await?
            .ok_or_else(|| Error::NoEncontrado("Un producto del conteo no existe".into()))?;
        validar_contado(&p, i.contado_mil)?;
        salida.push(diferencia(&p, i.contado_mil));
    }
    Ok(salida)
}

/// Motivo obligatorio, sin espacios sobrantes y de hasta 200 caracteres.
fn motivo_valido(motivo: &str) -> Resultado<String> {
    let motivo = motivo.trim();
    if motivo.is_empty() {
        return validacion("Escribe el motivo del ajuste");
    }
    if motivo.chars().count() > 200 {
        return validacion("El motivo no puede pasar de 200 caracteres");
    }
    Ok(motivo.to_owned())
}

/// Deja el stock de cada producto en lo contado, en una sola transacción. La diferencia se
/// calcula contra el stock al confirmar (si se vendió mientras se contaba, lo contado manda).
/// Los productos sin diferencia no generan movimiento. Devuelve las diferencias aplicadas.
pub async fn ajustar_conteo(
    pool: &SqlitePool,
    control: &ControlPin,
    items: &[Contado],
    motivo: &str,
    pin: Option<&str>,
) -> Resultado<Vec<Diferencia>> {
    let motivo = motivo_valido(motivo)?;
    // Valida antes del PIN, para no pedirlo en vano.
    previsualizar_conteo(pool, items).await?;
    seguridad::exigir(pool, control, Accion::AjustarInventario, pin).await?;

    let mut tx = pool.begin().await?;
    let mut salida = Vec::with_capacity(items.len());
    for i in items {
        let p = productos::por_id(&mut tx, i.producto_id)
            .await?
            .ok_or_else(|| Error::NoEncontrado("Un producto del conteo no existe".into()))?;
        let d = diferencia(&p, i.contado_mil);
        if d.diferencia_mil != 0 {
            productos::registrar_movimiento(
                &mut tx,
                &Movimiento {
                    producto_id: p.id,
                    delta_mil: d.diferencia_mil,
                    tipo: "ajuste",
                    venta_id: None,
                    compra_id: None,
                    nota: Some("Conteo"),
                    motivo: Some(&motivo),
                },
            )
            .await?;
        }
        salida.push(d);
    }
    tx.commit().await?;
    Ok(salida)
}

/// Por qué se pierde mercadería.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MotivoMerma {
    Vencido,
    Danado,
    Robo,
    ConsumoInterno,
    Otro,
}

impl MotivoMerma {
    pub fn nombre(self) -> &'static str {
        match self {
            Self::Vencido => "Vencido",
            Self::Danado => "Dañado",
            Self::Robo => "Robo",
            Self::ConsumoInterno => "Consumo interno",
            Self::Otro => "Otro",
        }
    }
}

/// Saca del stock lo que se perdió (movimiento 'merma'). "Otro" necesita el detalle.
/// Devuelve el stock que quedó.
pub async fn registrar_merma(
    pool: &SqlitePool,
    control: &ControlPin,
    producto_id: i64,
    cantidad_mil: i64,
    motivo: MotivoMerma,
    detalle: Option<&str>,
    pin: Option<&str>,
) -> Resultado<i64> {
    let detalle = detalle.map(str::trim).filter(|d| !d.is_empty());
    if detalle.is_some_and(|d| d.chars().count() > 200) {
        return validacion("El detalle no puede pasar de 200 caracteres");
    }
    let texto = match (motivo, detalle) {
        (MotivoMerma::Otro, None) => return validacion("Escribe el motivo de la merma"),
        (m, None) => m.nombre().to_owned(),
        (m, Some(d)) => format!("{}: {d}", m.nombre()),
    };
    let p = {
        let mut conn = pool.acquire().await?;
        productos::por_id(&mut conn, producto_id)
            .await?
            .ok_or_else(|| Error::NoEncontrado("El producto no existe".into()))?
    };
    cantidades::validar_venta(p.unidad, &p.nombre, cantidad_mil)?;
    seguridad::exigir(pool, control, Accion::AjustarInventario, pin).await?;

    let mut tx = pool.begin().await?;
    let stock = productos::registrar_movimiento(
        &mut tx,
        &Movimiento {
            producto_id,
            delta_mil: -cantidad_mil,
            tipo: "merma",
            venta_id: None,
            compra_id: None,
            nota: None,
            motivo: Some(&texto),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(stock)
}
