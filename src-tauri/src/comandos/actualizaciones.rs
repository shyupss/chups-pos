//! Actualizaciones firmadas (tauri-plugin-updater).
//!
//! - Se busca sola como máximo una vez al día, en segundo plano, con internet y nunca con un turno
//!   abierto (las reglas viven en `pos_core::servicios::actualizaciones`). Sin internet falla en
//!   silencio.
//! - Instalar hace antes un respaldo de la base y no se permite con un turno abierto: la
//!   instalación cierra la app. Con un turno abierto se ofrece "Instalar al cerrar el turno".
//! - La firma la verifica el plugin con la clave pública de tauri.conf.json.

use std::time::Duration;

use chrono::Utc;
use pos_core::servicios::actualizaciones as srv;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use super::{a_texto, ResultadoComando};
use crate::estado::EstadoApp;

/// Evento para la vista cuando una búsqueda automática encuentra una actualización.
pub const EVENTO_DISPONIBLE: &str = "actualizacion-disponible";
/// La búsqueda automática espera un poco al arrancar y después mira cada hora si toca.
const ESPERA_INICIAL: Duration = Duration::from_secs(90);
const CADA: Duration = Duration::from_secs(3600);
const TIEMPO_MAXIMO: Duration = Duration::from_secs(20);

/// Lo que el controlador recuerda entre llamadas (solo en memoria).
#[derive(Default)]
pub struct MemoriaActualizacion {
    pub disponible: Option<Update>,
    pub instalar_al_cerrar: bool,
    pub ocupada: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DatosActualizacion {
    pub version: String,
    /// Las notas de la versión (el CHANGELOG de esa versión).
    pub notas: Option<String>,
    pub fecha: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EstadoActualizacion {
    pub version_actual: String,
    pub disponible: Option<DatosActualizacion>,
    /// Segundos desde 1970 de la última búsqueda que llegó al servidor.
    pub ultima_busqueda: Option<i64>,
    pub instalar_al_cerrar: bool,
}

async fn estado_actual(estado: &EstadoApp) -> ResultadoComando<EstadoActualizacion> {
    let ultima_busqueda = srv::ultima_busqueda(&estado.pool).await.map_err(a_texto)?;
    let memoria = estado.actualizacion.lock().map_err(|e| e.to_string())?;
    Ok(EstadoActualizacion {
        version_actual: env!("CARGO_PKG_VERSION").to_owned(),
        disponible: memoria.disponible.as_ref().map(|u| DatosActualizacion {
            version: u.version.clone(),
            notas: u.body.clone().filter(|n| !n.trim().is_empty()),
            fecha: u.date.map(|d| d.to_string()),
        }),
        ultima_busqueda,
        instalar_al_cerrar: memoria.instalar_al_cerrar,
    })
}

/// Pregunta al servidor si hay una versión nueva. Si llegó al servidor, anota la búsqueda.
async fn buscar(app: &AppHandle) -> Result<Option<Update>, String> {
    let actualizador = app
        .updater_builder()
        .timeout(TIEMPO_MAXIMO)
        .build()
        .map_err(|e| e.to_string())?;
    let resultado = actualizador.check().await.map_err(|e| e.to_string())?;
    let estado = app.state::<EstadoApp>();
    // Si hubo respuesta (haya o no versión nueva), hoy ya no se busca más.
    let _ = srv::registrar_busqueda(&estado.pool, Utc::now().timestamp()).await;
    Ok(resultado)
}

fn guardar(estado: &EstadoApp, update: Option<Update>) {
    if let Ok(mut m) = estado.actualizacion.lock() {
        m.disponible = update;
    }
}

#[tauri::command]
pub async fn estado_actualizacion(
    estado: State<'_, EstadoApp>,
) -> ResultadoComando<EstadoActualizacion> {
    estado_actual(&estado).await
}

/// "Buscar actualizaciones" de Configuración > Acerca de: busca ahora, sin esperar el día, y si no
/// se puede (sin internet) lo dice.
#[tauri::command]
pub async fn buscar_actualizacion(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
) -> ResultadoComando<EstadoActualizacion> {
    let encontrada = buscar(&app).await.map_err(|_| {
        "No se pudo buscar actualizaciones: revisa la conexión a internet e inténtalo de nuevo."
            .to_owned()
    })?;
    guardar(&estado, encontrada);
    estado_actual(&estado).await
}

/// Deja pedido instalar la actualización cuando se cierre el turno (la vista lo hace al cerrarlo).
#[tauri::command]
pub async fn instalar_al_cerrar_turno(
    estado: State<'_, EstadoApp>,
    si: bool,
) -> ResultadoComando<EstadoActualizacion> {
    if let Ok(mut m) = estado.actualizacion.lock() {
        m.instalar_al_cerrar = si;
    }
    estado_actual(&estado).await
}

/// Respalda la base, descarga la actualización firmada y la instala. Con un turno abierto no se
/// permite. Al instalar, la app se cierra (el instalador la vuelve a abrir).
#[tauri::command]
pub async fn instalar_actualizacion(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
) -> ResultadoComando<()> {
    srv::exigir_sin_turno(&estado.pool).await.map_err(a_texto)?;
    let update = {
        let mut m = estado.actualizacion.lock().map_err(|e| e.to_string())?;
        if m.ocupada {
            return Err("Ya se está instalando la actualización".into());
        }
        let Some(u) = m.disponible.clone() else {
            return Err("No hay una actualización para instalar: busca de nuevo.".into());
        };
        m.ocupada = true;
        u
    };
    let resultado = async {
        // Primero el respaldo: si falla, no se actualiza.
        super::respaldos::respaldar_antes_de_actualizar(&estado).await?;
        update
            .download_and_install(|_, _| {}, || {})
            .await
            .map_err(|e| format!("No se pudo instalar la actualización: {e}"))
    }
    .await;
    if let Ok(mut m) = estado.actualizacion.lock() {
        m.ocupada = false;
        m.instalar_al_cerrar = false;
    }
    resultado?;
    // En Windows el instalador cierra la app antes de llegar aquí; en otros sistemas se reinicia.
    app.restart();
}

/// Búsqueda automática: espera al arrancar y después mira cada hora si toca (una vez al día, sin
/// turno abierto). Todo falla en silencio: sin internet, la caja ni se entera.
pub fn iniciar_busqueda_automatica(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(ESPERA_INICIAL).await;
        loop {
            let estado = app.state::<EstadoApp>();
            let ahora = Utc::now().timestamp();
            if srv::toca_buscar(&estado.pool, ahora).await.unwrap_or(false) {
                if let Ok(Some(update)) = buscar(&app).await {
                    guardar(&estado, Some(update));
                    let _ = app.emit(EVENTO_DISPONIBLE, ());
                }
            }
            tokio::time::sleep(CADA).await;
        }
    });
}
