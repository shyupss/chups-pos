use serde::Deserialize;
use sqlx::{SqliteConnection, SqlitePool};

use crate::error::validacion;
use crate::repositorios::categorias;
use crate::repositorios::productos::{self as repo, Campos, FiltroCategoria, Producto, Unidad};
use crate::servicios::cantidades;
use crate::{Error, Resultado};

/// Máximo de filas por página: la interfaz nunca pide el catálogo completo.
pub const LIMITE_MAXIMO: i64 = 200;

/// Lo que llega desde el formulario de producto. Stocks en milésimas.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatosProducto {
    pub codigo_barras: Option<String>,
    pub nombre: String,
    #[serde(default)]
    pub categoria_id: Option<i64>,
    #[serde(default)]
    pub unidad: Unidad,
    /// Precio por unidad o, en productos por kg, precio por kilo.
    pub precio_venta: i64,
    pub costo: Option<i64>,
    pub stock_minimo_mil: i64,
    /// Solo se usa al crear. Después el stock cambia con ventas y movimientos.
    #[serde(default)]
    pub stock_inicial_mil: i64,
}

struct Limpios {
    codigo: Option<String>,
    nombre: String,
}

fn validar(d: &DatosProducto) -> Resultado<Limpios> {
    let nombre = d.nombre.trim().to_owned();
    if nombre.is_empty() {
        return validacion("El nombre es obligatorio");
    }
    if nombre.chars().count() > 120 {
        return validacion("El nombre no puede pasar de 120 caracteres");
    }
    if d.precio_venta < 0 {
        return validacion("El precio no puede ser negativo");
    }
    if d.costo.is_some_and(|c| c < 0) {
        return validacion("El costo no puede ser negativo");
    }
    cantidades::validar_stock(d.unidad, d.stock_minimo_mil, "stock mínimo")?;
    let codigo = d
        .codigo_barras
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(str::to_owned);
    if codigo
        .as_deref()
        .is_some_and(|c| c.chars().any(char::is_whitespace))
    {
        return validacion("El código de barras no puede tener espacios");
    }
    Ok(Limpios { codigo, nombre })
}

/// La categoría elegida debe existir (la base lo exige, pero así el mensaje es claro).
async fn validar_categoria(
    conn: &mut SqliteConnection,
    categoria_id: Option<i64>,
) -> Resultado<()> {
    match categoria_id {
        Some(id) if !categorias::existe(conn, id).await? => {
            validacion("La categoría elegida no existe")
        }
        _ => Ok(()),
    }
}

pub async fn crear(pool: &SqlitePool, d: &DatosProducto) -> Resultado<Producto> {
    let l = validar(d)?;
    cantidades::validar_stock(d.unidad, d.stock_inicial_mil, "stock inicial")?;
    let mut tx = pool.begin().await?;
    validar_categoria(&mut tx, d.categoria_id).await?;
    let campos = Campos {
        codigo_barras: l.codigo.as_deref(),
        nombre: &l.nombre,
        categoria_id: d.categoria_id,
        unidad: d.unidad,
        precio_venta: d.precio_venta,
        costo: d.costo,
        stock_minimo_mil: d.stock_minimo_mil,
    };
    let id = repo::insertar(&mut tx, &campos).await?;
    if d.stock_inicial_mil > 0 {
        repo::mover_stock(
            &mut tx,
            id,
            d.stock_inicial_mil,
            "ajuste",
            None,
            Some("Stock inicial"),
        )
        .await?;
    }
    let producto = repo::por_id(&mut tx, id).await?.expect("recién insertado");
    tx.commit().await?;
    Ok(producto)
}

/// Edita los datos del producto. Si cambia el precio, queda en el historial de precios.
pub async fn editar(pool: &SqlitePool, id: i64, d: &DatosProducto) -> Resultado<Producto> {
    let l = validar(d)?;
    let mut tx = pool.begin().await?;
    let anterior = repo::por_id(&mut tx, id)
        .await?
        .ok_or_else(|| Error::NoEncontrado("El producto no existe".into()))?;
    validar_categoria(&mut tx, d.categoria_id).await?;
    let campos = Campos {
        codigo_barras: l.codigo.as_deref(),
        nombre: &l.nombre,
        categoria_id: d.categoria_id,
        unidad: d.unidad,
        precio_venta: d.precio_venta,
        costo: d.costo,
        stock_minimo_mil: d.stock_minimo_mil,
    };
    repo::actualizar(&mut tx, id, &campos).await?;
    if anterior.precio_venta != d.precio_venta {
        repo::registrar_cambio_precio(&mut tx, id, anterior.precio_venta, d.precio_venta).await?;
    }
    let producto = repo::por_id(&mut tx, id).await?.expect("existe");
    tx.commit().await?;
    Ok(producto)
}

/// Activa o desactiva. Un producto desactivado no aparece en la caja, pero sus ventas se conservan.
pub async fn cambiar_activo(pool: &SqlitePool, id: i64, activo: bool) -> Resultado<()> {
    if repo::cambiar_activo(pool, id, activo).await? == 0 {
        return Err(Error::NoEncontrado("El producto no existe".into()));
    }
    Ok(())
}

pub async fn buscar_por_codigo(pool: &SqlitePool, codigo: &str) -> Resultado<Option<Producto>> {
    let codigo = codigo.trim();
    if codigo.is_empty() {
        return Ok(None);
    }
    repo::por_codigo(pool, codigo).await
}

/// Búsqueda por nombre (sin importar tildes ni mayúsculas) o por inicio del código,
/// opcionalmente dentro de una categoría.
pub async fn buscar(
    pool: &SqlitePool,
    texto: &str,
    incluir_inactivos: bool,
    categoria: FiltroCategoria,
    limite: i64,
    desplazamiento: i64,
) -> Resultado<Vec<Producto>> {
    let limite = limite.clamp(1, LIMITE_MAXIMO);
    let desplazamiento = desplazamiento.max(0);
    let texto = texto.trim();
    let fts = consulta_fts(texto);
    let prefijo = parece_codigo(texto).then_some(texto);
    if !texto.is_empty() && fts.is_none() && prefijo.is_none() {
        return Ok(Vec::new());
    }
    repo::listar(
        pool,
        fts.as_deref(),
        prefijo,
        incluir_inactivos,
        categoria,
        limite,
        desplazamiento,
    )
    .await
}

/// Convierte "coca 1.5" en `"coca"* "1"* "5"*`: cada palabra como prefijo, todas obligatorias.
/// Se descartan los signos para que el texto del usuario nunca rompa la sintaxis de FTS5.
fn consulta_fts(texto: &str) -> Option<String> {
    let terminos: Vec<String> = texto
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\"*"))
        .collect();
    (!terminos.is_empty()).then(|| terminos.join(" "))
}

fn parece_codigo(texto: &str) -> bool {
    !texto.is_empty()
        && texto.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && texto.chars().any(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arma_consulta_fts_segura() {
        assert_eq!(
            consulta_fts("Coca 1.5"),
            Some(r#""Coca"* "1"* "5"*"#.into())
        );
        assert_eq!(consulta_fts(r#"pan" OR *"#), Some(r#""pan"* "OR"*"#.into()));
        assert_eq!(consulta_fts("  -- "), None);
    }

    #[test]
    fn detecta_codigos() {
        assert!(parece_codigo("7801610001196"));
        assert!(parece_codigo("ABC-12"));
        assert!(!parece_codigo("pan"));
        assert!(!parece_codigo("pan 1"));
    }
}
