// Ayudas compartidas por los tests de integración.
#![allow(dead_code)]

use pos_core::repositorios::productos::Producto;
use pos_core::servicios::productos::{self, DatosProducto};
use sqlx::SqlitePool;

pub fn datos(codigo: Option<&str>, nombre: &str, precio: i64, stock: i64) -> DatosProducto {
    DatosProducto {
        codigo_barras: codigo.map(str::to_owned),
        nombre: nombre.into(),
        precio_venta: precio,
        costo: None,
        stock_minimo: 0,
        stock_inicial: stock,
    }
}

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
