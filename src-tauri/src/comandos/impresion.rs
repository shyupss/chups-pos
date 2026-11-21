//! Impresora y cajón. Todo lo que toca hardware corre en un hilo aparte con un límite de
//! 10 segundos: una impresora apagada o un driver colgado nunca congelan la interfaz.

use std::time::Duration;

use pos_core::servicios::{config, impresion as srv};
use pos_hardware::{EstadoImpresora, Hardware, Impresora};
use serde::Serialize;
use tauri::State;

use super::{a_texto, ResultadoComando};
use crate::estado::EstadoApp;

const TIEMPO_MAXIMO: Duration = Duration::from_secs(10);

/// Ejecuta `tarea` con el hardware fuera de los hilos async, con límite de tiempo.
/// Si se pasa del límite, el hilo sigue esperando al driver, pero la app ya respondió.
/// El error de afuera es el del límite de tiempo; el de adentro, el del hardware.
async fn con_hardware_crudo<T, F>(
    estado: &EstadoApp,
    tarea: F,
) -> Result<pos_hardware::Resultado<T>, String>
where
    T: Send + 'static,
    F: FnOnce(&Hardware) -> pos_hardware::Resultado<T> + Send + 'static,
{
    let hw = estado.hardware.clone();
    let hilo = tauri::async_runtime::spawn_blocking(move || tarea(&hw));
    match tokio::time::timeout(TIEMPO_MAXIMO, hilo).await {
        Err(_) => Err(
            "La impresora no respondió en 10 segundos: revisa que esté encendida y conectada"
                .into(),
        ),
        Ok(Err(e)) => Err(e.to_string()),
        Ok(Ok(resultado)) => Ok(resultado),
    }
}

async fn con_hardware<T, F>(estado: &EstadoApp, tarea: F) -> ResultadoComando<T>
where
    T: Send + 'static,
    F: FnOnce(&Hardware) -> pos_hardware::Resultado<T> + Send + 'static,
{
    con_hardware_crudo(estado, tarea)
        .await?
        .map_err(|e| e.to_string())
}

pub(crate) async fn enviar(estado: &EstadoApp, trabajo: srv::Trabajo) -> ResultadoComando<()> {
    con_hardware(estado, move |hw| {
        hw.enviar(&trabajo.impresora, &trabajo.documento, &trabajo.bytes)
    })
    .await
}

/// Impresoras instaladas y, al final, "Archivo (pruebas)".
#[tauri::command]
pub async fn listar_impresoras(estado: State<'_, EstadoApp>) -> ResultadoComando<Vec<Impresora>> {
    con_hardware(&estado, |hw| hw.listar()).await
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoConfigurada {
    /// `None` si todavía no se eligió impresora.
    pub impresora: Option<String>,
    pub estado: Option<EstadoImpresora>,
}

/// Estado de la impresora elegida en Configuración (para el indicador de la barra lateral).
#[tauri::command]
pub async fn estado_impresora(estado: State<'_, EstadoApp>) -> ResultadoComando<EstadoConfigurada> {
    let cfg = config::obtener(&estado.pool).await.map_err(a_texto)?;
    let Some(impresora) = cfg.impresora else {
        return Ok(EstadoConfigurada {
            impresora: None,
            estado: None,
        });
    };
    let nombre = impresora.clone();
    let estado_impresora = match con_hardware_crudo(&estado, move |hw| hw.estado(&nombre)).await {
        Ok(Ok(e)) => e,
        // Desinstalada o sin conexión con el servidor de impresión.
        Ok(Err(pos_hardware::Error::NoEncontrada(_))) => EstadoImpresora::Desconectada,
        Ok(Err(e)) => EstadoImpresora::ConError(e.to_string()),
        Err(tiempo) => EstadoImpresora::ConError(tiempo),
    };
    Ok(EstadoConfigurada {
        impresora: Some(impresora),
        estado: Some(estado_impresora),
    })
}

/// Imprime la boleta de una venta. Sin `copias`, las de la configuración.
#[tauri::command]
pub async fn imprimir_boleta(
    estado: State<'_, EstadoApp>,
    venta_id: i64,
    copias: Option<u8>,
) -> ResultadoComando<()> {
    let trabajo = srv::trabajo_boleta(&estado.pool, venta_id, copias)
        .await
        .map_err(a_texto)?;
    enviar(&estado, trabajo).await
}

#[tauri::command]
pub async fn imprimir_prueba(estado: State<'_, EstadoApp>) -> ResultadoComando<()> {
    let trabajo = srv::trabajo_prueba(&estado.pool).await.map_err(a_texto)?;
    enviar(&estado, trabajo).await
}

/// Manda el pulso al cajón conectado a la impresora (pin y duración de la configuración).
#[tauri::command]
pub async fn abrir_cajon(estado: State<'_, EstadoApp>) -> ResultadoComando<()> {
    let trabajo = srv::trabajo_cajon(&estado.pool).await.map_err(a_texto)?;
    enviar(&estado, trabajo).await
}

/// Carpeta donde la impresora "Archivo (pruebas)" deja los .bin.
#[tauri::command]
pub fn carpeta_impresora_pruebas(estado: State<'_, EstadoApp>) -> String {
    estado
        .hardware
        .carpeta_pruebas()
        .to_string_lossy()
        .into_owned()
}
