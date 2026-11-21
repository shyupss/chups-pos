//! Ventas en espera: apartar el carrito de un cliente para atender a otro y retomarlo después.
//! No tocan stock ni folios; eso pasa recién al cobrar.

use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

use crate::error::validacion;
use crate::repositorios::espera::{self as repo, ResumenEspera};
use crate::repositorios::productos::{self, Producto};
use crate::servicios::ventas::ItemPedido;
use crate::{Error, Resultado};

/// Más que esto ya no es "un momento": hay que cobrar o descartar alguna.
pub const MAXIMO_EN_ESPERA: i64 = 20;

/// El carrito que se deja en espera. Cantidades en milésimas.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuevaEnEspera {
    /// Para reconocerla en la lista ("Señora del pan"). Opcional.
    pub nombre: Option<String>,
    pub items: Vec<ItemPedido>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LineaRecuperada {
    /// El producto con sus datos actuales (precio incluido).
    pub producto: Producto,
    pub cantidad_mil: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VentaRecuperada {
    pub lineas: Vec<LineaRecuperada>,
    /// Líneas que no volvieron al carrito porque el producto se desactivó.
    pub saltados: i64,
    /// Si se recuperó intercambiando, el id con que quedó en espera el carrito anterior.
    pub intercambio_id: Option<i64>,
}

fn no_existe() -> Error {
    Error::NoEncontrado("Esa venta en espera ya no existe".into())
}

fn limpiar_nombre(nombre: Option<&str>) -> Resultado<Option<String>> {
    let nombre = nombre.map(str::trim).filter(|n| !n.is_empty());
    if nombre.is_some_and(|n| n.chars().count() > 60) {
        return validacion("El nombre no puede pasar de 60 caracteres");
    }
    Ok(nombre.map(str::to_owned))
}

/// Guarda el carrito dentro de la transacción del llamador.
async fn guardar(conn: &mut SqliteConnection, v: &NuevaEnEspera) -> Resultado<i64> {
    if v.items.is_empty() {
        return validacion("No hay productos para dejar en espera");
    }
    let nombre = limpiar_nombre(v.nombre.as_deref())?;
    if repo::contar(conn).await? >= MAXIMO_EN_ESPERA {
        return validacion(format!(
            "Ya hay {MAXIMO_EN_ESPERA} ventas en espera. Cobra o descarta alguna antes de dejar otra."
        ));
    }
    for item in &v.items {
        if item.cantidad_mil <= 0 {
            return validacion("Cada producto debe tener una cantidad mayor que cero");
        }
        if productos::por_id(conn, item.producto_id).await?.is_none() {
            return Err(Error::NoEncontrado(
                "Un producto de la venta ya no existe".into(),
            ));
        }
    }
    let id = repo::insertar(conn, nombre.as_deref()).await?;
    for (orden, item) in (0_i64..).zip(&v.items) {
        repo::insertar_item(conn, id, item.producto_id, item.cantidad_mil, orden).await?;
    }
    Ok(id)
}

/// Deja un carrito en espera y devuelve su id.
pub async fn poner(pool: &SqlitePool, v: &NuevaEnEspera) -> Resultado<i64> {
    let mut tx = pool.begin().await?;
    let id = guardar(&mut tx, v).await?;
    tx.commit().await?;
    Ok(id)
}

pub async fn listar(pool: &SqlitePool) -> Resultado<Vec<ResumenEspera>> {
    repo::listar(pool).await
}

/// Devuelve las líneas de una venta en espera con los productos actuales y la borra, todo en
/// una transacción. Los productos desactivados se saltan y se informa cuántos fueron.
///
/// Con `intercambio`, el carrito que estaba en la caja queda en espera en la misma transacción:
/// nunca se pierde ninguno de los dos, y el límite se revisa con la recuperada ya borrada.
pub async fn recuperar(
    pool: &SqlitePool,
    id: i64,
    intercambio: Option<&NuevaEnEspera>,
) -> Resultado<VentaRecuperada> {
    let mut tx = pool.begin().await?;
    if !repo::existe(&mut tx, id).await? {
        return Err(no_existe());
    }
    let mut lineas = Vec::new();
    let mut saltados = 0;
    for (producto_id, cantidad_mil) in repo::items(&mut tx, id).await? {
        match productos::por_id(&mut tx, producto_id).await? {
            Some(producto) if producto.activo => lineas.push(LineaRecuperada {
                producto,
                cantidad_mil,
            }),
            _ => saltados += 1,
        }
    }
    repo::borrar(&mut tx, id).await?;
    let intercambio_id = match intercambio {
        Some(v) => Some(guardar(&mut tx, v).await?),
        None => None,
    };
    tx.commit().await?;
    Ok(VentaRecuperada {
        lineas,
        saltados,
        intercambio_id,
    })
}

pub async fn descartar(pool: &SqlitePool, id: i64) -> Resultado<()> {
    let mut conn = pool.acquire().await?;
    if repo::borrar(&mut conn, id).await? == 0 {
        return Err(no_existe());
    }
    Ok(())
}
