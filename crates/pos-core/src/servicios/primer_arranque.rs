//! Primer arranque: cuándo la app ofrece el asistente de configuración.
//!
//! El asistente se ofrece cuando la base está **vacía** (sin productos, ventas ni turnos) y todavía
//! no se terminó. Una base con datos, como la de la fase 6, nunca lo pide; se puede volver a abrir a
//! mano desde Configuración. No se mira el nombre del negocio: si se cierra la app a la mitad, al
//! volver el asistente sigue pendiente hasta terminarlo.

use sqlx::SqlitePool;

use crate::repositorios::config as repo_config;
use crate::Resultado;

/// Clave de la tabla config que marca el asistente como terminado (o saltado del todo).
const TERMINADO: &str = "asistente_terminado";

/// ¿Hay que mostrar el asistente al abrir la app?
pub async fn pendiente(pool: &SqlitePool) -> Resultado<bool> {
    let mut conn = pool.acquire().await?;
    let claves = repo_config::leer_todo(&mut conn).await?;
    if claves.get(TERMINADO).is_some_and(|v| v == "1") {
        return Ok(false);
    }
    // Cualquier dato del negocio (aunque sea un turno o un producto inactivo) cuenta como uso.
    let con_datos: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM productos)
             OR EXISTS (SELECT 1 FROM ventas)
             OR EXISTS (SELECT 1 FROM turnos)",
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok(!con_datos)
}

/// Marca el asistente como terminado: no vuelve a aparecer solo. Sirve igual si se omitieron todos
/// los pasos; no cambia nada más de la configuración.
pub async fn terminar(pool: &SqlitePool) -> Resultado<()> {
    let mut conn = pool.acquire().await?;
    repo_config::guardar(&mut conn, TERMINADO, "1").await
}
