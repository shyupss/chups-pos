//! Reglas de las actualizaciones automáticas (el controlador hace la búsqueda y la instalación).
//!
//! - Se busca como máximo una vez al día: solo cuenta una búsqueda que llegó al servidor.
//! - Nunca con un turno abierto: ni se busca solo ni se instala.
//! - Sin internet no pasa nada: la búsqueda falla en silencio y se reintenta más tarde.

use sqlx::SqlitePool;

use crate::error::validacion;
use crate::repositorios::config as repo_config;
use crate::repositorios::turnos as repo_turnos;
use crate::Resultado;

const ULTIMA_BUSQUEDA: &str = "actualizacion_ultima_busqueda";
/// Cada cuántos segundos se puede volver a buscar sola: una vez al día.
pub const INTERVALO_BUSQUEDA: i64 = 24 * 3600;

/// Segundos (desde 1970) de la última búsqueda que llegó al servidor.
pub async fn ultima_busqueda(pool: &SqlitePool) -> Resultado<Option<i64>> {
    let mut conn = pool.acquire().await?;
    let claves = repo_config::leer_todo(&mut conn).await?;
    Ok(claves
        .get(ULTIMA_BUSQUEDA)
        .and_then(|v| v.trim().parse::<i64>().ok()))
}

pub async fn registrar_busqueda(pool: &SqlitePool, ahora: i64) -> Resultado<()> {
    let mut conn = pool.acquire().await?;
    repo_config::guardar(&mut conn, ULTIMA_BUSQUEDA, &ahora.to_string()).await
}

async fn hay_turno_abierto(pool: &SqlitePool) -> Resultado<bool> {
    let mut conn = pool.acquire().await?;
    Ok(repo_turnos::id_abierto(&mut conn).await?.is_some())
}

/// ¿Toca buscar una actualización sola? Sí si no hay turno abierto y pasó un día desde la última
/// búsqueda que llegó al servidor (o nunca se buscó).
pub async fn toca_buscar(pool: &SqlitePool, ahora: i64) -> Resultado<bool> {
    if hay_turno_abierto(pool).await? {
        return Ok(false);
    }
    Ok(match ultima_busqueda(pool).await? {
        None => true,
        Some(ultima) => ahora - ultima >= INTERVALO_BUSQUEDA,
    })
}

/// Para instalar: con un turno abierto no se puede (la instalación cierra la app).
pub async fn exigir_sin_turno(pool: &SqlitePool) -> Resultado<()> {
    if hay_turno_abierto(pool).await? {
        return validacion(
            "Cierra el turno de caja antes de instalar la actualización: instalarla cierra CHUPS POS.",
        );
    }
    Ok(())
}
