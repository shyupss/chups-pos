//! Compras a proveedores: suman stock (movimientos 'compra' con su compra_id) y dejan en cada
//! producto el último costo unitario. Si se pagan con efectivo de la caja, el retiro queda en el
//! turno abierto en la misma transacción.
//!
//! Anular una compra no la borra: la compensa con movimientos negativos y, si se pagó con la
//! caja, devuelve el dinero al turno actual con un ingreso. Pide el PIN de "ajustar inventario".

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::boleta::formato::pesos;
use crate::error::validacion;
use crate::repositorios::compras::{self as repo, FilaCompra, FiltroCompras};
use crate::repositorios::productos::{self, Movimiento, Unidad};
use crate::servicios::proveedores::opcional;
use crate::servicios::seguridad::{self, Accion, ControlPin};
use crate::servicios::turnos::{self, TipoMovimiento, MONTO_MAXIMO};
use crate::servicios::{cantidades, historial};
use crate::{Error, Resultado};

/// Una línea de la compra. Costo por unidad o, en productos por kg, por kilo.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineaCompra {
    pub producto_id: i64,
    pub cantidad_mil: i64,
    pub costo_unitario: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuevaCompra {
    pub proveedor_id: Option<i64>,
    pub documento: Option<String>,
    pub nota: Option<String>,
    pub lineas: Vec<LineaCompra>,
    /// Se pagó con efectivo de la caja: un retiro en el turno abierto.
    #[serde(default)]
    pub pagada_con_caja: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ItemCompra {
    pub producto_id: i64,
    pub nombre: String,
    pub unidad: Unidad,
    pub cantidad_mil: i64,
    pub costo_unitario: i64,
    pub subtotal: i64,
    pub stock_mil: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Compra {
    pub id: i64,
    pub fecha: String,
    pub proveedor_id: Option<i64>,
    pub proveedor: Option<String>,
    pub documento: Option<String>,
    pub total: i64,
    pub nota: Option<String>,
    pub pagada_con_caja: bool,
    pub anulada_en: Option<String>,
    pub motivo_anulacion: Option<String>,
    pub lineas: i64,
}

impl From<FilaCompra> for Compra {
    fn from(f: FilaCompra) -> Self {
        Compra {
            id: f.id,
            fecha: f.fecha,
            proveedor_id: f.proveedor_id,
            proveedor: f.proveedor,
            documento: f.documento,
            total: f.total,
            nota: f.nota,
            pagada_con_caja: f.pagada_con_caja,
            anulada_en: f.anulada_en,
            motivo_anulacion: f.motivo_anulacion,
            lineas: f.lineas,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DetalleCompra {
    pub compra: Compra,
    pub items: Vec<ItemCompra>,
    /// Si se anulara ahora, estos productos quedarían con stock negativo.
    pub quedarian_negativos: Vec<String>,
}

/// Subtotal de una línea: costo × cantidad, redondeado al peso (como en la venta).
fn subtotal(costo: i64, cantidad_mil: i64) -> Resultado<i64> {
    cantidades::subtotal(costo, cantidad_mil)
        .filter(|s| *s <= MONTO_MAXIMO)
        .ok_or_else(|| Error::Validacion("El monto de la compra es demasiado grande".into()))
}

/// Registra la compra en una transacción. Si se paga con la caja, exige turno abierto y respeta
/// el PIN de los retiros. Devuelve el detalle de la compra guardada.
pub async fn registrar(
    pool: &SqlitePool,
    control: &ControlPin,
    c: &NuevaCompra,
    pin: Option<&str>,
) -> Resultado<DetalleCompra> {
    if c.lineas.is_empty() {
        return validacion("La compra no tiene productos");
    }
    let mut vistos = HashSet::new();
    if c.lineas.iter().any(|l| !vistos.insert(l.producto_id)) {
        return validacion("Cada producto va una sola vez en la compra: suma las cantidades");
    }
    if c.lineas
        .iter()
        .any(|l| !(0..=MONTO_MAXIMO).contains(&l.costo_unitario))
    {
        return validacion("El costo debe estar entre $0 y $100.000.000");
    }
    let documento = opcional(c.documento.as_deref(), 40, "El n° de documento")?;
    let nota = opcional(c.nota.as_deref(), 200, "La nota")?;

    // Productos, cantidades y total, antes de pedir el PIN.
    let mut total: i64 = 0;
    let mut subtotales = Vec::with_capacity(c.lineas.len());
    let proveedor = {
        let mut conn = pool.acquire().await?;
        for l in &c.lineas {
            let p = productos::por_id(&mut conn, l.producto_id)
                .await?
                .ok_or_else(|| Error::NoEncontrado("Un producto de la compra no existe".into()))?;
            cantidades::validar_venta(p.unidad, &p.nombre, l.cantidad_mil)?;
            let s = subtotal(l.costo_unitario, l.cantidad_mil)?;
            total = total
                .checked_add(s)
                .filter(|t| *t <= MONTO_MAXIMO)
                .ok_or_else(|| {
                    Error::Validacion("El monto de la compra es demasiado grande".into())
                })?;
            subtotales.push(s);
        }
        match c.proveedor_id {
            Some(id) => Some(
                crate::repositorios::proveedores::por_id(&mut conn, id)
                    .await?
                    .filter(|p| p.activo)
                    .ok_or_else(|| {
                        Error::Validacion("El proveedor no existe o está desactivado".into())
                    })?,
            ),
            None => None,
        }
    };
    let retiro = c.pagada_con_caja && total > 0;
    if retiro {
        if turnos::estado_caja(pool).await?.turno.is_none() {
            return Err(turnos::sin_caja("pagar la compra con efectivo de la caja"));
        }
        seguridad::exigir(pool, control, Accion::RetiroEfectivo, pin).await?;
    }

    let mut tx = pool.begin().await?;
    let compra_id = repo::insertar(
        &mut tx,
        &repo::NuevaCompra {
            proveedor_id: c.proveedor_id,
            documento: documento.as_deref(),
            total,
            nota: nota.as_deref(),
            pagada_con_caja: c.pagada_con_caja,
        },
    )
    .await?;
    let referencia = format!("Compra N° {compra_id}");
    for (l, s) in c.lineas.iter().zip(&subtotales) {
        repo::insertar_item(
            &mut tx,
            compra_id,
            l.producto_id,
            l.cantidad_mil,
            l.costo_unitario,
            *s,
        )
        .await?;
        productos::registrar_movimiento(
            &mut tx,
            &Movimiento {
                producto_id: l.producto_id,
                delta_mil: l.cantidad_mil,
                tipo: "compra",
                venta_id: None,
                compra_id: Some(compra_id),
                nota: Some(&referencia),
                motivo: None,
            },
        )
        .await?;
        productos::actualizar_costo(&mut tx, l.producto_id, l.costo_unitario).await?;
    }
    if retiro {
        let motivo = match &proveedor {
            Some(p) => format!("Compra a {}", p.nombre),
            None => "Compra sin proveedor".to_owned(),
        };
        let m = turnos::movimiento_en(
            &mut tx,
            TipoMovimiento::Retiro,
            total,
            &motivo,
            "pagar la compra con efectivo de la caja",
        )
        .await?;
        repo::fijar_movimiento_caja(&mut tx, compra_id, m.id).await?;
    }
    tx.commit().await?;
    detalle(pool, compra_id).await
}

pub async fn detalle(pool: &SqlitePool, id: i64) -> Resultado<DetalleCompra> {
    let mut conn = pool.acquire().await?;
    let compra: Compra = repo::por_id(&mut conn, id)
        .await?
        .ok_or_else(|| Error::NoEncontrado("La compra no existe".into()))?
        .into();
    let items: Vec<ItemCompra> = repo::items(&mut conn, id)
        .await?
        .into_iter()
        .map(|i| ItemCompra {
            producto_id: i.producto_id,
            nombre: i.nombre,
            unidad: i.unidad,
            cantidad_mil: i.cantidad_mil,
            costo_unitario: i.costo_unitario,
            subtotal: i.subtotal,
            stock_mil: i.stock_mil,
        })
        .collect();
    let quedarian_negativos = if compra.anulada_en.is_none() {
        items
            .iter()
            .filter(|i| i.stock_mil - i.cantidad_mil < 0)
            .map(|i| i.nombre.clone())
            .collect()
    } else {
        Vec::new()
    };
    Ok(DetalleCompra {
        compra,
        items,
        quedarian_negativos,
    })
}

/// Filtros del historial de compras. Fechas como días locales (hasta incluido).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiltrosCompras {
    pub proveedor_id: Option<i64>,
    pub desde: Option<String>,
    pub hasta: Option<String>,
}

pub async fn listar(
    pool: &SqlitePool,
    f: &FiltrosCompras,
    limite: i64,
    desplazamiento: i64,
) -> Resultado<Vec<Compra>> {
    let desde = f.desde.as_deref().map(historial::dia).transpose()?;
    let hasta = f.hasta.as_deref().map(historial::dia).transpose()?;
    Ok(repo::listar(
        pool,
        &FiltroCompras {
            proveedor_id: f.proveedor_id,
            desde_utc: desde.map(historial::inicio_del_dia),
            hasta_utc: hasta
                .and_then(|h| h.succ_opt())
                .map(historial::inicio_del_dia),
        },
        limite.clamp(1, 200),
        desplazamiento.max(0),
    )
    .await?
    .into_iter()
    .map(Compra::from)
    .collect())
}

/// Anula la compra: movimientos 'compra' negativos que compensan el stock (aunque algún
/// producto quede negativo: el detalle lo advierte antes) y, si se pagó con la caja, un ingreso
/// en el turno actual. Pide el PIN de "ajustar inventario".
pub async fn anular(
    pool: &SqlitePool,
    control: &ControlPin,
    id: i64,
    motivo: &str,
    pin: Option<&str>,
) -> Resultado<DetalleCompra> {
    let Some(motivo) = opcional(Some(motivo), 200, "El motivo")? else {
        return validacion("Escribe el motivo de la anulación");
    };
    let d = detalle(pool, id).await?;
    if d.compra.anulada_en.is_some() {
        return validacion("La compra ya está anulada");
    }
    let devolver = d.compra.pagada_con_caja && d.compra.total > 0;
    if devolver && turnos::estado_caja(pool).await?.turno.is_none() {
        return Err(turnos::sin_caja("devolver a la caja el pago de la compra"));
    }
    seguridad::exigir(pool, control, Accion::AjustarInventario, pin).await?;

    let mut tx = pool.begin().await?;
    if !repo::anular(&mut tx, id, &motivo).await? {
        return validacion("La compra ya está anulada");
    }
    let nota = format!("Anulación de la compra N° {id}");
    for i in &d.items {
        productos::registrar_movimiento(
            &mut tx,
            &Movimiento {
                producto_id: i.producto_id,
                delta_mil: -i.cantidad_mil,
                tipo: "compra",
                venta_id: None,
                compra_id: Some(id),
                nota: Some(&nota),
                motivo: Some(&motivo),
            },
        )
        .await?;
    }
    if devolver {
        turnos::movimiento_en(
            &mut tx,
            TipoMovimiento::Ingreso,
            d.compra.total,
            &format!("{nota} ({})", pesos(d.compra.total)),
            "devolver a la caja el pago de la compra",
        )
        .await?;
    }
    tx.commit().await?;
    detalle(pool, id).await
}
