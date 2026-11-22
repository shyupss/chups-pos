//! Estado de la licencia y qué se puede hacer en cada uno.
//!
//! - **Prueba**: [`DIAS_PRUEBA`] días desde el primer arranque (clave `dias_prueba`), con todo.
//! - **Activa**: hay una licencia válida para este equipo. Es permanente: ningún reloj, fecha ni
//!   actualización la desactiva.
//! - **Gracia**: [`DIAS_GRACIA`] días después de la prueba, con todo y un aviso visible.
//! - **Consulta**: después de la gracia. Se ve y exporta todo, se respalda y restaura, se cobran
//!   fiados y se cierra el turno abierto, pero no se abre un turno nuevo ni se registran ventas.
//!   La venta en curso no se corta: el turno que estaba abierto al terminar la gracia puede seguir
//!   vendiendo hasta [`MARGEN_TURNO`] después.
//!
//! Lo deciden [`exigir_venta`] y [`exigir_abrir_turno`], que llaman los comandos: la vista solo
//! muestra [`estado`].
//!
//! Reloj: se guarda el mayor instante visto (al abrir y en cada venta). Los días de prueba se
//! cuentan contra ese instante, así que atrasar el reloj no los devuelve; si el reloj está más de
//! [`ATRASO_AVISO`] atrás, el estado lo avisa.

use std::path::PathBuf;

use chrono::{DateTime, SecondsFormat, Utc};
use pos_licencia::{Huella, Licencia, Solicitud};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::repositorios::config as repo_config;
use crate::repositorios::turnos as repo_turnos;
use crate::{Error, Resultado};

pub const DIAS_PRUEBA: i64 = 15;
pub const DIAS_GRACIA: i64 = 7;
/// Hasta cuánto después del fin de la gracia puede seguir vendiendo el turno que estaba abierto.
pub const MARGEN_TURNO: i64 = 24 * 3600;
/// Un reloj atrasado más que esto respecto del mayor instante visto se avisa.
pub const ATRASO_AVISO: i64 = 48 * 3600;
const DIA: i64 = 86_400;

// Claves de la tabla config (solo este archivo las usa; no van en `Config`, así guardar la
// configuración desde la vista no las toca).
const CODIGO: &str = "licencia_codigo";
const EQUIPO: &str = "licencia_equipo";
const ACTIVADA_EN: &str = "licencia_activada_en";
const PRUEBA_INICIO: &str = "prueba_inicio";
const CLAVE_DIAS_PRUEBA: &str = "dias_prueba";
const RELOJ_MAXIMO: &str = "reloj_maximo";

/// Lo que el servicio necesita saber del equipo y de la app. Lo arma el controlador al arrancar.
#[derive(Debug, Clone)]
pub struct Contexto {
    pub clave_publica: [u8; 32],
    pub huella: Huella,
    /// Archivo con el inicio de la prueba, en la carpeta de datos. `None` en los tests.
    pub archivo_prueba: Option<PathBuf>,
    /// Versión de la app, para el código de solicitud.
    pub version: String,
    /// Cómo contactar al vendedor (se fija al compilar).
    pub contacto: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Modo {
    Prueba,
    Activa,
    Gracia,
    Consulta,
}

/// Los datos de la licencia activa, para Configuración > Licencia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatosLicencia {
    pub id: String,
    pub negocio: String,
    pub rut: Option<String>,
    pub emitida_en: String,
    pub equipos: u8,
    /// Cuándo se activó en este equipo (ISO 8601, UTC).
    pub activada_en: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoLicencia {
    pub modo: Modo,
    /// En prueba, los días que quedan; en gracia, los días de gracia que quedan.
    pub dias_restantes: Option<i64>,
    pub dias_prueba: i64,
    pub inicio_prueba: String,
    pub fin_prueba: String,
    pub fin_gracia: String,
    /// El reloj del equipo está más de 48 horas atrás del mayor instante visto.
    pub reloj_atrasado: bool,
    pub licencia: Option<DatosLicencia>,
    /// Hay una licencia guardada que no sirve en este equipo (por ejemplo, la base se copió de
    /// otro computador). El modo sigue la prueba.
    pub error_licencia: Option<String>,
    /// Para pedir una licencia atada a este equipo.
    pub codigo_solicitud: String,
    pub contacto: String,
    /// Se pueden abrir turnos y registrar ventas.
    pub puede_vender: bool,
}

/// Lo guardado en la tabla config, ya leído.
struct Guardado {
    codigo: Option<String>,
    equipo: Option<Huella>,
    activada_en: Option<String>,
    prueba_inicio: Option<i64>,
    dias_prueba: i64,
    reloj_maximo: Option<i64>,
}

async fn leer_guardado(pool: &SqlitePool) -> Resultado<Guardado> {
    let mut conn = pool.acquire().await?;
    let claves = repo_config::leer_todo(&mut conn).await?;
    let texto = |k: &str| claves.get(k).filter(|v| !v.is_empty()).cloned();
    let numero = |k: &str| texto(k).and_then(|v| v.trim().parse::<i64>().ok());
    Ok(Guardado {
        codigo: texto(CODIGO),
        equipo: texto(EQUIPO).and_then(|h| Huella::desde_texto(&h)),
        activada_en: texto(ACTIVADA_EN),
        prueba_inicio: numero(PRUEBA_INICIO),
        // Acotado: una clave editada a mano no da una prueba eterna.
        dias_prueba: numero(CLAVE_DIAS_PRUEBA)
            .unwrap_or(DIAS_PRUEBA)
            .clamp(1, 60),
        reloj_maximo: numero(RELOJ_MAXIMO),
    })
}

fn iso(segundos: i64) -> String {
    DateTime::<Utc>::from_timestamp(segundos, 0)
        .unwrap_or_default()
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// La licencia guardada, si es válida para este equipo; si no, el motivo.
fn licencia_valida(g: &Guardado, ctx: &Contexto) -> Option<Result<Licencia, String>> {
    let codigo = g.codigo.as_deref()?;
    Some(
        pos_licencia::leer(codigo, &ctx.clave_publica)
            .and_then(|lic| {
                pos_licencia::validar_equipo(&lic, &ctx.huella, g.equipo.as_ref()).map(|_| lic)
            })
            .map_err(|e| e.to_string()),
    )
}

fn calcular(g: &Guardado, ctx: &Contexto, ahora: i64) -> EstadoLicencia {
    // El tiempo no retrocede para la prueba: se cuenta contra el mayor instante visto.
    let efectivo = g.reloj_maximo.map_or(ahora, |m| m.max(ahora));
    let reloj_atrasado = g.reloj_maximo.is_some_and(|m| ahora < m - ATRASO_AVISO);
    let inicio = g.prueba_inicio.unwrap_or(ahora);
    let fin_prueba = inicio + g.dias_prueba * DIA;
    let fin_gracia = fin_prueba + DIAS_GRACIA * DIA;
    let dias_hasta = |fin: i64| (fin - efectivo + DIA - 1).div_euclid(DIA);

    let (licencia, error_licencia) = match licencia_valida(g, ctx) {
        Some(Ok(lic)) => (Some(lic), None),
        Some(Err(e)) => (None, Some(e)),
        None => (None, None),
    };
    let (modo, dias_restantes) = if licencia.is_some() {
        (Modo::Activa, None)
    } else if efectivo < fin_prueba {
        (Modo::Prueba, Some(dias_hasta(fin_prueba)))
    } else if efectivo < fin_gracia {
        (Modo::Gracia, Some(dias_hasta(fin_gracia)))
    } else {
        (Modo::Consulta, None)
    };

    EstadoLicencia {
        modo,
        dias_restantes,
        dias_prueba: g.dias_prueba,
        inicio_prueba: iso(inicio),
        fin_prueba: iso(fin_prueba),
        fin_gracia: iso(fin_gracia),
        reloj_atrasado,
        licencia: licencia.map(|l| DatosLicencia {
            id: l.id,
            negocio: l.negocio,
            rut: l.rut,
            emitida_en: l.emitida_en,
            equipos: l.equipos,
            activada_en: g.activada_en.clone(),
        }),
        error_licencia,
        codigo_solicitud: Solicitud::nueva(ctx.huella, &ctx.version).como_texto(),
        contacto: ctx.contacto.clone(),
        puede_vender: modo != Modo::Consulta,
    }
}

/// El estado de la licencia en `ahora` (segundos desde 1970, UTC).
pub async fn estado(pool: &SqlitePool, ctx: &Contexto, ahora: i64) -> Resultado<EstadoLicencia> {
    Ok(calcular(&leer_guardado(pool).await?, ctx, ahora))
}

/// Al arrancar: fija el inicio de la prueba (el más antiguo entre la base y el archivo aparte, o
/// ahora si es el primer arranque), lo deja en los dos lugares y anota el instante.
pub async fn iniciar(pool: &SqlitePool, ctx: &Contexto, ahora: i64) -> Resultado<EstadoLicencia> {
    let g = leer_guardado(pool).await?;
    let del_archivo = ctx
        .archivo_prueba
        .as_ref()
        .and_then(|r| std::fs::read_to_string(r).ok())
        .and_then(|t| pos_licencia::leer_marca_prueba(&t, &ctx.huella));
    let inicio = [g.prueba_inicio, del_archivo]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(ahora);

    let mut conn = pool.acquire().await?;
    if g.prueba_inicio != Some(inicio) {
        repo_config::guardar(&mut conn, PRUEBA_INICIO, &inicio.to_string()).await?;
    }
    if let Some(ruta) = &ctx.archivo_prueba {
        if del_archivo != Some(inicio) {
            // Si no se puede escribir (carpeta de solo lectura), la base basta.
            let _ = std::fs::write(ruta, pos_licencia::marca_prueba(inicio, &ctx.huella));
        }
    }
    drop(conn);
    registrar_instante(pool, ahora).await?;
    estado(pool, ctx, ahora).await
}

/// Guarda `ahora` si es el mayor instante visto. Una sola sentencia: se llama en cada venta.
pub async fn registrar_instante(pool: &SqlitePool, ahora: i64) -> Resultado<()> {
    sqlx::query(
        "INSERT INTO config (clave, valor) VALUES (?, ?)
         ON CONFLICT (clave) DO UPDATE SET valor = excluded.valor
         WHERE CAST(config.valor AS INTEGER) < CAST(excluded.valor AS INTEGER)",
    )
    .bind(RELOJ_MAXIMO)
    .bind(ahora.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

/// Activa una licencia (el texto o el contenido del archivo). Si viene sin equipo, se ata a este.
/// Reemplaza la que hubiera.
pub async fn activar(
    pool: &SqlitePool,
    ctx: &Contexto,
    texto: &str,
    ahora: i64,
) -> Resultado<EstadoLicencia> {
    let lic = pos_licencia::leer(texto, &ctx.clave_publica)
        .map_err(|e| Error::Licencia(e.to_string()))?;
    let g = leer_guardado(pool).await?;
    // La atadura guardada solo cuenta para la misma licencia (una base copiada de otro equipo).
    let misma = g
        .codigo
        .as_deref()
        .and_then(|c| pos_licencia::leer(c, &ctx.clave_publica).ok())
        .is_some_and(|anterior| anterior.id == lic.id);
    let atada = if misma { g.equipo.as_ref() } else { None };
    let huella = pos_licencia::validar_equipo(&lic, &ctx.huella, atada)
        .map_err(|e| Error::Licencia(e.to_string()))?;

    let mut tx = pool.begin().await?;
    repo_config::guardar(&mut tx, CODIGO, texto.trim()).await?;
    repo_config::guardar(&mut tx, EQUIPO, &huella.como_texto()).await?;
    if !misma || g.activada_en.is_none() {
        repo_config::guardar(&mut tx, ACTIVADA_EN, &iso(ahora)).await?;
    }
    tx.commit().await?;
    estado(pool, ctx, ahora).await
}

/// Para confirmar una venta. En consulta se rechaza, salvo en el turno que estaba abierto al
/// terminar la gracia (hasta [`MARGEN_TURNO`] después): una venta en curso no se corta.
pub async fn exigir_venta(pool: &SqlitePool, ctx: &Contexto, ahora: i64) -> Resultado<()> {
    let g = leer_guardado(pool).await?;
    let e = calcular(&g, ctx, ahora);
    if e.puede_vender {
        return Ok(());
    }
    let efectivo = g.reloj_maximo.map_or(ahora, |m| m.max(ahora));
    let fin_gracia = DateTime::parse_from_rfc3339(&e.fin_gracia)
        .map(|d| d.timestamp())
        .unwrap_or(ahora);
    let mut conn = pool.acquire().await?;
    let abierto_antes = repo_turnos::abierto(&mut conn)
        .await?
        .and_then(|t| DateTime::parse_from_rfc3339(&t.abierto_en).ok())
        .is_some_and(|abierto| abierto.timestamp() < fin_gracia);
    if abierto_antes && efectivo < fin_gracia + MARGEN_TURNO {
        return Ok(());
    }
    Err(Error::Licencia(
        "La prueba de CHUPS POS terminó: para registrar ventas activa una licencia en \
         Configuración > Licencia. Puedes seguir viendo, exportando y respaldando tus datos."
            .into(),
    ))
}

/// Para abrir un turno: en consulta no se puede.
pub async fn exigir_abrir_turno(pool: &SqlitePool, ctx: &Contexto, ahora: i64) -> Resultado<()> {
    if estado(pool, ctx, ahora).await?.puede_vender {
        return Ok(());
    }
    Err(Error::Licencia(
        "La prueba de CHUPS POS terminó: para abrir la caja activa una licencia en \
         Configuración > Licencia. Puedes cerrar el turno, cobrar fiados y ver, exportar y \
         respaldar tus datos."
            .into(),
    ))
}

/// Segundos desde 1970 del reloj del equipo.
pub fn ahora() -> i64 {
    Utc::now().timestamp()
}
