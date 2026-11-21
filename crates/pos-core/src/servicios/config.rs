use serde::Serialize;
use sqlx::{SqliteConnection, SqlitePool};

use crate::repositorios::config as repo;
use crate::Resultado;

/// Clave de config: "1" redondea el efectivo a la decena, "0" no. Si falta, se redondea.
pub const REDONDEO_EFECTIVO: &str = "redondeo_efectivo";

/// Ajustes que la caja necesita conocer.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigCaja {
    pub redondeo_efectivo: bool,
}

pub async fn redondeo_efectivo_activo(conn: &mut SqliteConnection) -> Resultado<bool> {
    Ok(repo::leer(conn, REDONDEO_EFECTIVO).await?.as_deref() != Some("0"))
}

pub async fn config_caja(pool: &SqlitePool) -> Resultado<ConfigCaja> {
    let mut conn = pool.acquire().await?;
    Ok(ConfigCaja {
        redondeo_efectivo: redondeo_efectivo_activo(&mut conn).await?,
    })
}

pub async fn cambiar_redondeo_efectivo(pool: &SqlitePool, activo: bool) -> Resultado<()> {
    repo::guardar(pool, REDONDEO_EFECTIVO, if activo { "1" } else { "0" }).await
}
