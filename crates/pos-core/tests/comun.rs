// Ayudas compartidas por los tests de integración.
#![allow(dead_code)]

use pos_core::repositorios::productos::{Producto, Unidad};
use pos_core::servicios::productos::{self, DatosProducto};
use sqlx::SqlitePool;

/// Datos de un producto por unidad. `stock` va en unidades (se guarda en milésimas).
pub fn datos(codigo: Option<&str>, nombre: &str, precio: i64, stock: i64) -> DatosProducto {
    DatosProducto {
        codigo_barras: codigo.map(str::to_owned),
        nombre: nombre.into(),
        categoria_id: None,
        unidad: Unidad::Unidad,
        precio_venta: precio,
        costo: None,
        stock_minimo_mil: 0,
        stock_inicial_mil: stock * 1000,
    }
}

/// Producto por unidad con `stock` unidades.
pub async fn crear(
    pool: &SqlitePool,
    codigo: &str,
    nombre: &str,
    precio: i64,
    stock: i64,
) -> Producto {
    productos::crear(pool, &datos(Some(codigo), nombre, precio, stock))
        .await
        .unwrap()
}

/// Producto por kg: `precio_kg` es el precio por kilo y `stock_mil` el stock en gramos.
pub async fn crear_kg(
    pool: &SqlitePool,
    codigo: &str,
    nombre: &str,
    precio_kg: i64,
    stock_mil: i64,
) -> Producto {
    let mut d = datos(Some(codigo), nombre, precio_kg, 0);
    d.unidad = Unidad::Kg;
    d.stock_inicial_mil = stock_mil;
    productos::crear(pool, &d).await.unwrap()
}
