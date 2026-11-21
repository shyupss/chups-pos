//! Lectura de etiquetas de balanza en la caja: producto por kg y su peso, sin diálogo.

use serde::Serialize;
use sqlx::SqlitePool;

use crate::balanza::{self, ReglasBalanza, ValorBalanza};
use crate::error::validacion;
use crate::repositorios::productos::{self, Producto, Unidad};
use crate::servicios::{cantidades, config};
use crate::{Error, Resultado};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LecturaBalanza {
    pub producto: Producto,
    /// Gramos de la etiqueta (o calculados desde el precio).
    pub cantidad_mil: i64,
}

/// `Ok(None)` si los códigos de balanza están desactivados o el código no tiene ese formato.
/// Busca el producto por su código tal cual y, si no aparece, sin ceros a la izquierda
/// ("00123" → "123"): dos consultas por índice como máximo.
pub async fn leer(pool: &SqlitePool, codigo: &str) -> Resultado<Option<LecturaBalanza>> {
    let cfg = config::obtener(pool).await?;
    if !cfg.balanza_activa {
        return Ok(None);
    }
    let Some(etiqueta) = balanza::analizar(codigo, &ReglasBalanza::desde(&cfg))? else {
        return Ok(None);
    };
    let sin_ceros = etiqueta.codigo_producto.trim_start_matches('0');
    let producto = match productos::por_codigo(pool, &etiqueta.codigo_producto).await? {
        Some(p) => Some(p),
        None if !sin_ceros.is_empty() && sin_ceros != etiqueta.codigo_producto => {
            productos::por_codigo(pool, sin_ceros).await?
        }
        None => None,
    };
    let producto = producto.ok_or_else(|| {
        Error::NoEncontrado(format!(
            "La etiqueta de balanza es del producto {}, que no está registrado (o está desactivado)",
            etiqueta.codigo_producto
        ))
    })?;
    if producto.unidad != Unidad::Kg {
        return validacion(format!(
            "{} se vende por unidad: una etiqueta de balanza solo sirve para productos por kg",
            producto.nombre
        ));
    }
    let gramos = match etiqueta.valor {
        ValorBalanza::Gramos(g) => g,
        ValorBalanza::Pesos(precio) => balanza::gramos_desde_precio(precio, producto.precio_venta)
            .ok_or_else(|| {
                Error::Validacion(format!("{} no tiene precio por kg", producto.nombre))
            })?,
    };
    cantidades::validar_venta(producto.unidad, &producto.nombre, gramos)?;
    Ok(Some(LecturaBalanza {
        producto,
        cantidad_mil: gramos,
    }))
}
