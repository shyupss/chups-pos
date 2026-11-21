use sqlx::SqlitePool;

use crate::error::validacion;
use crate::repositorios::categorias::{self as repo, Categoria};
use crate::{Error, Resultado};

fn no_existe() -> Error {
    Error::NoEncontrado("La categoría no existe".into())
}

/// Nombre limpio y válido. La unicidad sin importar mayúsculas la asegura la base (COLLATE NOCASE).
fn validar_nombre(nombre: &str) -> Resultado<String> {
    let nombre = nombre.trim();
    if nombre.is_empty() {
        return validacion("El nombre de la categoría es obligatorio");
    }
    if nombre.chars().count() > 60 {
        return validacion("El nombre de la categoría no puede pasar de 60 caracteres");
    }
    Ok(nombre.to_owned())
}

/// Categorías por nombre, con la cantidad de productos activos de cada una.
pub async fn listar(pool: &SqlitePool) -> Resultado<Vec<Categoria>> {
    repo::listar(pool).await
}

pub async fn crear(pool: &SqlitePool, nombre: &str) -> Resultado<Categoria> {
    let nombre = validar_nombre(nombre)?;
    let id = repo::insertar(pool, &nombre).await?;
    repo::por_id(pool, id).await?.ok_or_else(no_existe)
}

pub async fn renombrar(pool: &SqlitePool, id: i64, nombre: &str) -> Resultado<Categoria> {
    let nombre = validar_nombre(nombre)?;
    if repo::renombrar(pool, id, &nombre).await? == 0 {
        return Err(no_existe());
    }
    repo::por_id(pool, id).await?.ok_or_else(no_existe)
}

/// Solo se elimina una categoría vacía, para no dejar productos apuntando a nada.
pub async fn eliminar(pool: &SqlitePool, id: i64) -> Resultado<()> {
    let mut tx = pool.begin().await?;
    let productos = repo::contar_productos(&mut tx, id).await?;
    if productos > 0 {
        let cuantos = if productos == 1 {
            "1 producto".to_owned()
        } else {
            format!("{productos} productos")
        };
        return Err(Error::Conflicto(format!(
            "No se puede eliminar: la categoría tiene {cuantos} (contando los desactivados). \
             Cámbialos de categoría primero."
        )));
    }
    if repo::eliminar(&mut tx, id).await? == 0 {
        return Err(no_existe());
    }
    tx.commit().await?;
    Ok(())
}
