//! Revisión de la base tras un cierre brusco y restauración del último respaldo sano.
//!
//! La revisión (`PRAGMA quick_check`) se hace al arrancar solo si la vez anterior no se cerró bien
//! (ver `lib.rs`); aquí se consulta su resultado y se restaura si hace falta.

use std::path::PathBuf;

use chrono::Utc;
use pos_core::servicios::respaldos::Respaldo;
use pos_core::servicios::seguridad::{self, Accion};
use pos_core::servicios::{config, integridad as srv};
use pos_core::Error;
use serde::Serialize;
use tauri::{AppHandle, State};

use super::{ResultadoComando, ResultadoProtegido};
use crate::estado::EstadoApp;

/// Lo que encontró la revisión del arranque.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EstadoIntegridad {
    /// La vez anterior no se cerró bien (corte de luz, cierre a la fuerza).
    pub cierre_brusco: bool,
    /// Lo que dijo `quick_check`; vacío si la base está sana (o si no hacía falta revisarla).
    pub problemas: Vec<String>,
    /// El respaldo más nuevo que pasa la verificación, para ofrecer restaurarlo.
    pub ultimo_sano: Option<Respaldo>,
}

#[tauri::command]
pub async fn estado_integridad(estado: State<'_, EstadoApp>) -> ResultadoComando<EstadoIntegridad> {
    Ok(estado.integridad.lock().map_err(|e| e.to_string())?.clone())
}

/// Restaura el último respaldo sano sobre la base dañada. Antes guarda una copia de la dañada
/// (`chups-pos-danada-…` en la carpeta de respaldos) y después la app se reinicia. Pide el PIN, si
/// hay uno; si la base está tan dañada que no se puede leer, no se puede revisar y se sigue.
#[tauri::command]
pub async fn restaurar_ultimo_sano(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
    pin: Option<String>,
) -> ResultadoProtegido<()> {
    // Una base ilegible no deja leer el PIN (es justo el caso que se quiere arreglar): solo un PIN
    // pedido y mal puesto detiene la restauración.
    if let Err(Error::Pin(f)) = seguridad::exigir(
        &estado.pool,
        &estado.pin,
        Accion::RestaurarRespaldo,
        pin.as_deref(),
    )
    .await
    {
        return Err(Error::Pin(f).into());
    }
    let carpeta = config::obtener(&estado.pool)
        .await
        .ok()
        .and_then(|c| c.respaldos_carpeta)
        .map(PathBuf::from)
        .unwrap_or_else(|| estado.carpeta_respaldos.clone());
    let sano = srv::ultimo_sano(&carpeta)
        .await?
        .ok_or_else(|| Error::Validacion("No hay ningún respaldo sano para restaurar".into()))?;
    srv::guardar_danada(
        &estado.ruta_db,
        &carpeta,
        &Utc::now().format("%Y-%m-%d-%H%M").to_string(),
    )?;
    tracing::warn!("restaurando el último respaldo sano ({})", sano.nombre);
    super::respaldos::reemplazar_y_reiniciar(&app, &estado, PathBuf::from(sano.ruta)).await
}
