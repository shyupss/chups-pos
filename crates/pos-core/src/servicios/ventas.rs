use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::validacion;
use crate::repositorios::{productos, ventas};
use crate::{Error, Resultado};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MedioPago {
    Efectivo,
    Debito,
    Credito,
    Transferencia,
    Fiado,
}

impl MedioPago {
    pub fn como_texto(self) -> &'static str {
        match self {
            MedioPago::Efectivo => "efectivo",
            MedioPago::Debito => "debito",
            MedioPago::Credito => "credito",
            MedioPago::Transferencia => "transferencia",
            MedioPago::Fiado => "fiado",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemPedido {
    pub producto_id: i64,
    pub cantidad: i64,
}

/// Lo que envía la caja al cobrar. Los precios NO vienen de la interfaz:
/// se leen de la base al confirmar, para que el total siempre sea el correcto.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuevaVenta {
    pub items: Vec<ItemPedido>,
    pub medio_pago: MedioPago,
    /// Solo en efectivo: lo que entregó el cliente.
    pub monto_recibido: Option<i64>,
    #[serde(default)]
    pub descuento: i64,
    pub cliente_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LineaVendida {
    pub producto_id: i64,
    pub nombre: String,
    pub precio_unitario: i64,
    pub cantidad: i64,
    pub subtotal: i64,
    pub stock_resultante: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VentaConfirmada {
    pub id: i64,
    pub folio: i64,
    pub fecha_hora: String,
    pub subtotal: i64,
    pub descuento: i64,
    pub total: i64,
    pub medio_pago: MedioPago,
    pub monto_recibido: Option<i64>,
    pub vuelto: Option<i64>,
    pub lineas: Vec<LineaVendida>,
}

const MAX_CANTIDAD: i64 = 100_000;

fn desborde() -> Error {
    Error::Validacion("El monto de la venta es demasiado grande".into())
}

/// Confirma una venta en UNA transacción: cabecera, líneas, descuento de stock y
/// movimientos. Si algo falla (o se corta la luz) no queda nada a medias.
///
/// El stock puede quedar negativo: en un negocio real no se debe bloquear una venta
/// porque el inventario esté desactualizado. Se corrige con un ajuste.
pub async fn confirmar(pool: &SqlitePool, v: &NuevaVenta) -> Resultado<VentaConfirmada> {
    if v.items.is_empty() {
        return validacion("La venta no tiene productos");
    }
    // Junta líneas repetidas del mismo producto, conservando el orden de primera aparición.
    let mut orden: Vec<i64> = Vec::new();
    let mut cantidades: BTreeMap<i64, i64> = BTreeMap::new();
    for item in &v.items {
        if item.cantidad <= 0 || item.cantidad > MAX_CANTIDAD {
            return validacion("Cada producto debe tener una cantidad entre 1 y 100.000");
        }
        let acumulado = cantidades.entry(item.producto_id).or_insert_with(|| {
            orden.push(item.producto_id);
            0
        });
        *acumulado += item.cantidad;
    }
    if v.medio_pago == MedioPago::Fiado && v.cliente_id.is_none() {
        return validacion("Para fiar hay que elegir un cliente");
    }
    if v.medio_pago != MedioPago::Fiado && v.cliente_id.is_some() {
        return validacion("Solo las ventas fiadas llevan cliente");
    }

    let mut tx = pool.begin().await?;

    let mut lineas = Vec::with_capacity(orden.len());
    let mut subtotal: i64 = 0;
    for producto_id in &orden {
        let cantidad = cantidades[producto_id];
        let p = productos::por_id(&mut tx, *producto_id)
            .await?
            .filter(|p| p.activo)
            .ok_or_else(|| {
                Error::NoEncontrado("Un producto de la venta ya no existe o fue desactivado".into())
            })?;
        let sub = p.precio_venta.checked_mul(cantidad).ok_or_else(desborde)?;
        subtotal = subtotal.checked_add(sub).ok_or_else(desborde)?;
        lineas.push(LineaVendida {
            producto_id: p.id,
            nombre: p.nombre,
            precio_unitario: p.precio_venta,
            cantidad,
            subtotal: sub,
            stock_resultante: 0,
        });
    }

    if v.descuento < 0 || v.descuento > subtotal {
        return validacion("El descuento debe estar entre 0 y el subtotal");
    }
    let total = subtotal - v.descuento;

    let (monto_recibido, vuelto) = match v.medio_pago {
        MedioPago::Efectivo => {
            let recibido = v.monto_recibido.unwrap_or(total);
            if recibido < total {
                return validacion("El monto recibido es menor que el total");
            }
            (Some(recibido), Some(recibido - total))
        }
        MedioPago::Fiado => (None, None),
        _ => (Some(total), Some(0)),
    };
    let estado = if v.medio_pago == MedioPago::Fiado {
        "fiada"
    } else {
        "pagada"
    };

    let cabecera = ventas::insertar_cabecera(
        &mut tx,
        &ventas::NuevaCabecera {
            subtotal,
            descuento: v.descuento,
            total,
            medio_pago: v.medio_pago.como_texto(),
            monto_recibido,
            vuelto,
            estado,
            cliente_id: v.cliente_id,
        },
    )
    .await?;

    for l in &mut lineas {
        ventas::insertar_item(
            &mut tx,
            cabecera.id,
            l.producto_id,
            &l.nombre,
            l.precio_unitario,
            l.cantidad,
            l.subtotal,
        )
        .await?;
        l.stock_resultante = productos::mover_stock(
            &mut tx,
            l.producto_id,
            -l.cantidad,
            "venta",
            Some(cabecera.id),
            None,
        )
        .await?;
    }

    tx.commit().await?;

    Ok(VentaConfirmada {
        id: cabecera.id,
        folio: cabecera.folio,
        fecha_hora: cabecera.fecha_hora,
        subtotal,
        descuento: v.descuento,
        total,
        medio_pago: v.medio_pago,
        monto_recibido,
        vuelto,
        lineas,
    })
}
