//! Clientes a los que se les fía: datos, límite de crédito y deuda.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::validacion;
use crate::repositorios::clientes::{self as repo, Campos, FilaCliente};
use crate::servicios::proveedores::opcional;
use crate::servicios::turnos::MONTO_MAXIMO;
use crate::{rut, Error, Resultado};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Cliente {
    pub id: i64,
    pub nombre: String,
    pub telefono: Option<String>,
    pub rut: Option<String>,
    pub direccion: Option<String>,
    pub nota: Option<String>,
    /// `None` = sin límite.
    pub limite_credito: Option<i64>,
    pub activo: bool,
    pub creado_en: Option<String>,
    /// Lo que debe hoy (saldos de sus fiadas pendientes).
    pub deuda: i64,
    /// Cuánto más se le puede fiar; `None` si no tiene límite.
    pub disponible: Option<i64>,
}

impl From<FilaCliente> for Cliente {
    fn from(f: FilaCliente) -> Self {
        Cliente {
            disponible: f.limite_credito.map(|l| (l - f.deuda).max(0)),
            id: f.id,
            nombre: f.nombre,
            telefono: f.telefono,
            rut: f.rut,
            direccion: f.direccion,
            nota: f.nota,
            limite_credito: f.limite_credito,
            activo: f.activo,
            creado_en: f.creado_en,
            deuda: f.deuda,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatosCliente {
    pub nombre: String,
    pub telefono: Option<String>,
    #[serde(default)]
    pub rut: Option<String>,
    #[serde(default)]
    pub direccion: Option<String>,
    #[serde(default)]
    pub nota: Option<String>,
    #[serde(default)]
    pub limite_credito: Option<i64>,
}

struct Limpios {
    nombre: String,
    telefono: Option<String>,
    rut: Option<String>,
    direccion: Option<String>,
    nota: Option<String>,
    limite: Option<i64>,
}

fn validar(d: &DatosCliente) -> Resultado<Limpios> {
    let Some(nombre) = opcional(Some(&d.nombre), 80, "El nombre")? else {
        return validacion("El nombre del cliente es obligatorio");
    };
    let rut = match d.rut.as_deref().map(str::trim).filter(|r| !r.is_empty()) {
        Some(r) => Some(rut::normalizar(r)?),
        None => None,
    };
    if d.limite_credito
        .is_some_and(|l| !(0..=MONTO_MAXIMO).contains(&l))
    {
        return validacion("El límite de crédito debe estar entre $0 y $100.000.000");
    }
    Ok(Limpios {
        nombre,
        telefono: opcional(d.telefono.as_deref(), 30, "El teléfono")?,
        rut,
        direccion: opcional(d.direccion.as_deref(), 120, "La dirección")?,
        nota: opcional(d.nota.as_deref(), 300, "La nota")?,
        limite: d.limite_credito,
    })
}

impl Limpios {
    fn campos(&self) -> Campos<'_> {
        Campos {
            nombre: &self.nombre,
            telefono: self.telefono.as_deref(),
            rut: self.rut.as_deref(),
            direccion: self.direccion.as_deref(),
            nota: self.nota.as_deref(),
            limite_credito: self.limite,
        }
    }
}

pub async fn obtener(pool: &SqlitePool, id: i64) -> Resultado<Cliente> {
    let mut conn = pool.acquire().await?;
    repo::por_id(&mut conn, id)
        .await?
        .map(Cliente::from)
        .ok_or_else(|| Error::NoEncontrado("El cliente no existe".into()))
}

/// Por nombre, teléfono o RUT (con o sin puntos). Solo activos.
pub async fn buscar(pool: &SqlitePool, texto: &str, limite: i64) -> Resultado<Vec<Cliente>> {
    Ok(repo::buscar(pool, texto.trim(), limite.clamp(1, 50))
        .await?
        .into_iter()
        .map(Cliente::from)
        .collect())
}

pub async fn crear(pool: &SqlitePool, d: &DatosCliente) -> Resultado<Cliente> {
    let l = validar(d)?;
    let id = {
        let mut conn = pool.acquire().await?;
        repo::insertar(&mut conn, &l.campos()).await?
    };
    obtener(pool, id).await
}

pub async fn editar(pool: &SqlitePool, id: i64, d: &DatosCliente) -> Resultado<Cliente> {
    let l = validar(d)?;
    {
        let mut conn = pool.acquire().await?;
        if repo::actualizar(&mut conn, id, &l.campos()).await? == 0 {
            return Err(Error::NoEncontrado("El cliente no existe".into()));
        }
    }
    obtener(pool, id).await
}

/// Desactivar solo si no debe nada; activar siempre.
pub async fn cambiar_activo(pool: &SqlitePool, id: i64, activo: bool) -> Resultado<Cliente> {
    let c = obtener(pool, id).await?;
    if !activo && c.deuda > 0 {
        return validacion(format!(
            "{} todavía debe {}: no se puede desactivar",
            c.nombre,
            crate::boleta::formato::pesos(c.deuda)
        ));
    }
    let mut conn = pool.acquire().await?;
    repo::cambiar_activo(&mut conn, id, activo).await?;
    drop(conn);
    obtener(pool, id).await
}
