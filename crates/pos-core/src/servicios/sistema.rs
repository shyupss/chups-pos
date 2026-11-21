use serde::Serialize;
use sqlx::SqlitePool;

use crate::{db, repositorios, Resultado};

/// Resumen del estado de la app, para la pantalla de inicio y diagnóstico.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EstadoSistema {
    pub version: String,
    pub ruta_base_datos: String,
    pub version_esquema: i64,
    pub total_productos: i64,
    pub ms_inicio: u64,
}

pub async fn estado(
    pool: &SqlitePool,
    version: &str,
    ruta_base_datos: &str,
    ms_inicio: u64,
) -> Resultado<EstadoSistema> {
    Ok(EstadoSistema {
        version: version.to_owned(),
        ruta_base_datos: ruta_base_datos.to_owned(),
        version_esquema: db::version_esquema(pool).await?,
        total_productos: repositorios::productos::contar_activos(pool).await?,
        ms_inicio,
    })
}
