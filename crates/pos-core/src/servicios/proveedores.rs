//! Proveedores: a quién se le compra. Se crean, editan y desactivan; nunca se borran (un
//! trigger lo impide si tienen compras).

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::validacion;
use crate::repositorios::proveedores::{self as repo, Campos, FilaProveedor};
use crate::{rut, Error, Resultado};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Proveedor {
    pub id: i64,
    pub nombre: String,
    pub rut: Option<String>,
    pub telefono: Option<String>,
    pub contacto: Option<String>,
    pub nota: Option<String>,
    pub activo: bool,
    /// Compras vigentes (sin las anuladas) y la fecha de la última.
    pub compras: i64,
    pub ultima_compra: Option<String>,
}

impl From<FilaProveedor> for Proveedor {
    fn from(f: FilaProveedor) -> Self {
        Proveedor {
            id: f.id,
            nombre: f.nombre,
            rut: f.rut,
            telefono: f.telefono,
            contacto: f.contacto,
            nota: f.nota,
            activo: f.activo,
            compras: f.compras,
            ultima_compra: f.ultima_compra,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatosProveedor {
    pub nombre: String,
    pub rut: Option<String>,
    pub telefono: Option<String>,
    pub contacto: Option<String>,
    pub nota: Option<String>,
}

/// Texto opcional: sin espacios sobrantes, vacío = no hay, y con largo máximo.
pub(crate) fn opcional(texto: Option<&str>, maximo: usize, que: &str) -> Resultado<Option<String>> {
    let texto = texto.map(str::trim).filter(|t| !t.is_empty());
    if texto.is_some_and(|t| t.chars().count() > maximo) {
        return validacion(format!("{que} no puede pasar de {maximo} caracteres"));
    }
    Ok(texto.map(str::to_owned))
}

struct Limpios {
    nombre: String,
    rut: Option<String>,
    telefono: Option<String>,
    contacto: Option<String>,
    nota: Option<String>,
}

fn validar(d: &DatosProveedor) -> Resultado<Limpios> {
    let Some(nombre) = opcional(Some(&d.nombre), 80, "El nombre")? else {
        return validacion("El nombre del proveedor es obligatorio");
    };
    let rut = match d.rut.as_deref().map(str::trim).filter(|r| !r.is_empty()) {
        Some(r) => Some(rut::normalizar(r)?),
        None => None,
    };
    Ok(Limpios {
        nombre,
        rut,
        telefono: opcional(d.telefono.as_deref(), 30, "El teléfono")?,
        contacto: opcional(d.contacto.as_deref(), 80, "El contacto")?,
        nota: opcional(d.nota.as_deref(), 300, "La nota")?,
    })
}

impl Limpios {
    fn campos(&self) -> Campos<'_> {
        Campos {
            nombre: &self.nombre,
            rut: self.rut.as_deref(),
            telefono: self.telefono.as_deref(),
            contacto: self.contacto.as_deref(),
            nota: self.nota.as_deref(),
        }
    }
}

pub async fn obtener(pool: &SqlitePool, id: i64) -> Resultado<Proveedor> {
    let mut conn = pool.acquire().await?;
    repo::por_id(&mut conn, id)
        .await?
        .map(Proveedor::from)
        .ok_or_else(|| Error::NoEncontrado("El proveedor no existe".into()))
}

pub async fn listar(
    pool: &SqlitePool,
    texto: &str,
    incluir_inactivos: bool,
    limite: i64,
    desplazamiento: i64,
) -> Resultado<Vec<Proveedor>> {
    let texto = texto.trim();
    Ok(repo::listar(
        pool,
        (!texto.is_empty()).then_some(texto),
        incluir_inactivos,
        limite.clamp(1, 200),
        desplazamiento.max(0),
    )
    .await?
    .into_iter()
    .map(Proveedor::from)
    .collect())
}

pub async fn crear(pool: &SqlitePool, d: &DatosProveedor) -> Resultado<Proveedor> {
    let l = validar(d)?;
    let id = {
        let mut conn = pool.acquire().await?;
        repo::insertar(&mut conn, &l.campos()).await?
    };
    obtener(pool, id).await
}

pub async fn editar(pool: &SqlitePool, id: i64, d: &DatosProveedor) -> Resultado<Proveedor> {
    let l = validar(d)?;
    {
        let mut conn = pool.acquire().await?;
        if repo::actualizar(&mut conn, id, &l.campos()).await? == 0 {
            return Err(Error::NoEncontrado("El proveedor no existe".into()));
        }
    }
    obtener(pool, id).await
}

/// Un proveedor desactivado no aparece al registrar compras; sus compras se conservan.
pub async fn cambiar_activo(pool: &SqlitePool, id: i64, activo: bool) -> Resultado<Proveedor> {
    if repo::cambiar_activo(pool, id, activo).await? == 0 {
        return Err(Error::NoEncontrado("El proveedor no existe".into()));
    }
    obtener(pool, id).await
}
