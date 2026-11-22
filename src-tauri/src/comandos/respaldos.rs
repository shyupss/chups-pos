//! Respaldos: automáticos (al abrir la app si el último tiene más de 24 horas, y al cerrar un
//! turno), manuales ("Respaldar ahora" y "Respaldar en…") y restaurar. Nunca bloquean la caja:
//! corren en segundo plano y solo avisan si fallan (evento "respaldo-fallido").

use std::path::PathBuf;

use chrono::Utc;
use pos_core::servicios::config;
use pos_core::servicios::respaldos::{self as srv, InfoRespaldo, Respaldo};
use pos_core::servicios::seguridad::{self, Accion};
use pos_core::zona::ZonaLocal;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use super::{a_texto, ResultadoComando, ResultadoProtegido};
use crate::estado::EstadoApp;

/// Evento para la vista cuando un respaldo automático falla (con el mensaje).
pub const EVENTO_FALLO: &str = "respaldo-fallido";

/// La carpeta configurada, o Respaldos junto a los datos de la app.
async fn carpeta(estado: &EstadoApp) -> ResultadoComando<(PathBuf, usize)> {
    let cfg = config::obtener(&estado.pool).await.map_err(a_texto)?;
    let c = cfg
        .respaldos_carpeta
        .map(PathBuf::from)
        .unwrap_or_else(|| estado.carpeta_respaldos.clone());
    Ok((c, usize::from(cfg.respaldos_conservar)))
}

/// Respalda en la carpeta configurada y deja los últimos N.
async fn respaldar_y_rotar(estado: &EstadoApp) -> ResultadoComando<Respaldo> {
    let (c, conservar) = carpeta(estado).await?;
    let r = srv::respaldar(&estado.pool, &c, &ZonaLocal, Utc::now(), None)
        .await
        .map_err(a_texto)?;
    let rotar = c.clone();
    tauri::async_runtime::spawn_blocking(move || srv::rotar(&rotar, conservar))
        .await
        .map_err(|e| e.to_string())?
        .map_err(a_texto)?;
    Ok(r)
}

/// El respaldo que se hace antes de instalar una actualización: si falla, no se actualiza.
pub(crate) async fn respaldar_antes_de_actualizar(estado: &EstadoApp) -> Result<(), String> {
    respaldar_y_rotar(estado)
        .await
        .map(|_| ())
        .map_err(|e| format!("No se instaló la actualización porque falló el respaldo previo: {e}"))
}

/// Respaldo automático en segundo plano. `siempre`: aunque el último sea reciente (al cerrar un
/// turno). Si falla, lo guarda para Configuración > Datos y avisa a la vista.
pub(crate) fn automatico(app: &AppHandle, siempre: bool) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let estado = app.state::<EstadoApp>();
        let resultado = async {
            if !siempre {
                let (c, _) = carpeta(&estado).await?;
                if !srv::toca_automatico(&c, Utc::now()) {
                    return Ok(());
                }
            }
            respaldar_y_rotar(&estado).await.map(|_| ())
        }
        .await;
        let mut error = estado.error_respaldo.lock().expect("mutex sano");
        match resultado {
            Ok(()) => *error = None,
            Err(e) => {
                tracing::error!("el respaldo automático falló: {e}");
                let mensaje = format!("No se pudo hacer el respaldo automático: {e}");
                *error = Some(mensaje.clone());
                let _ = app.emit(EVENTO_FALLO, mensaje);
            }
        }
    });
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoRespaldos {
    pub ruta_base: String,
    pub bytes_base: u64,
    pub carpeta: String,
    pub carpeta_predeterminada: String,
    pub conservar: usize,
    /// Del más nuevo al más antiguo.
    pub respaldos: Vec<Respaldo>,
    /// El último respaldo automático que falló (o una restauración que no se pudo hacer).
    pub error: Option<String>,
}

#[tauri::command]
pub async fn estado_respaldos(estado: State<'_, EstadoApp>) -> ResultadoComando<EstadoRespaldos> {
    let (c, conservar) = carpeta(&estado).await?;
    let ruta = estado.ruta_db.clone();
    let lista = c.clone();
    let (bytes_base, respaldos) = tauri::async_runtime::spawn_blocking(move || {
        (srv::tamano_base(&ruta), srv::listar(&lista))
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(EstadoRespaldos {
        ruta_base: estado.ruta_db.to_string_lossy().into_owned(),
        bytes_base,
        carpeta: c.to_string_lossy().into_owned(),
        carpeta_predeterminada: estado.carpeta_respaldos.to_string_lossy().into_owned(),
        conservar,
        respaldos: respaldos.map_err(a_texto)?,
        error: estado.error_respaldo.lock().expect("mutex sano").clone(),
    })
}

#[tauri::command]
pub async fn respaldar_ahora(estado: State<'_, EstadoApp>) -> ResultadoComando<Respaldo> {
    let r = respaldar_y_rotar(&estado).await?;
    *estado.error_respaldo.lock().expect("mutex sano") = None;
    Ok(r)
}

/// Respalda en otra carpeta (un pendrive). Ahí no se borra nada. `None` si se cancela.
#[tauri::command]
pub async fn respaldar_en(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
) -> ResultadoComando<Option<Respaldo>> {
    let Some(c) = elegir_carpeta(&app, "Carpeta para el respaldo").await? else {
        return Ok(None);
    };
    Ok(Some(
        srv::respaldar(&estado.pool, &c, &ZonaLocal, Utc::now(), None)
            .await
            .map_err(a_texto)?,
    ))
}

async fn elegir_carpeta(
    app: &AppHandle,
    titulo: &'static str,
) -> ResultadoComando<Option<PathBuf>> {
    let app = app.clone();
    let elegida = tauri::async_runtime::spawn_blocking(move || {
        app.dialog().file().set_title(titulo).blocking_pick_folder()
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(elegida.and_then(|c| c.into_path().ok()))
}

/// Para Configuración: elegir la carpeta de los respaldos automáticos.
#[tauri::command]
pub async fn elegir_carpeta_respaldos(app: AppHandle) -> ResultadoComando<Option<String>> {
    Ok(elegir_carpeta(&app, "Carpeta de los respaldos")
        .await?
        .map(|c| c.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn abrir_carpeta_respaldos(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
) -> ResultadoComando<()> {
    let (c, _) = carpeta(&estado).await?;
    let _ = std::fs::create_dir_all(&c);
    app.opener()
        .open_path(c.to_string_lossy(), None::<&str>)
        .map_err(|e| format!("No se pudo abrir la carpeta: {e}"))
}

/// Elige el archivo a restaurar. `None` si se cancela.
#[tauri::command]
pub async fn elegir_respaldo(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
) -> ResultadoComando<Option<String>> {
    let (c, _) = carpeta(&estado).await?;
    let elegida = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Respaldo para restaurar")
            .set_directory(c)
            .add_filter("Respaldo de CHUPS POS", &["db"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(elegida
        .and_then(|r| r.into_path().ok())
        .map(|r| r.to_string_lossy().into_owned()))
}

/// Revisa el archivo antes de restaurar: fecha, versión, productos y última venta.
#[tauri::command]
pub async fn inspeccionar_respaldo(ruta: String) -> ResultadoComando<InfoRespaldo> {
    srv::inspeccionar(&PathBuf::from(ruta))
        .await
        .map_err(a_texto)
}

/// Restaura el respaldo (PIN "restaurar respaldo", siempre activado): lo revisa de nuevo,
/// respalda la base actual, cierra el pool, pone el respaldo en su lugar y reinicia la app (al
/// abrirse, las migraciones lo ponen al día si es más antiguo).
#[tauri::command]
pub async fn restaurar_respaldo(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
    ruta: String,
    pin: Option<String>,
) -> ResultadoProtegido<()> {
    seguridad::exigir(
        &estado.pool,
        &estado.pin,
        Accion::RestaurarRespaldo,
        pin.as_deref(),
    )
    .await?;
    let nueva = PathBuf::from(&ruta);
    srv::inspeccionar(&nueva).await?;
    let (c, _) = carpeta(&estado).await?;
    srv::respaldar(
        &estado.pool,
        &c,
        &ZonaLocal,
        Utc::now(),
        Some("antes-de-restaurar"),
    )
    .await?;

    reemplazar_y_reiniciar(&app, &estado, nueva).await
}

/// Cierra la base, pone `nueva` en su lugar y reinicia la app. Desde aquí la app se reinicia pase
/// lo que pase: sin pool no puede seguir. Lo usan restaurar un respaldo y restaurar el último sano.
pub(crate) async fn reemplazar_y_reiniciar(
    app: &AppHandle,
    estado: &EstadoApp,
    nueva: PathBuf,
) -> ! {
    estado.lectura.close().await;
    estado.pool.close().await;
    let destino = estado.ruta_db.clone();
    let reemplazo =
        tauri::async_runtime::spawn_blocking(move || srv::reemplazar_base(&destino, &nueva))
            .await
            .map_err(|e| e.to_string());
    if let Err(e) | Ok(Err(e)) = reemplazo.map(|r| r.map_err(|e| e.to_string())) {
        // La base anterior quedó intacta: se avisa al volver a abrir.
        let _ = std::fs::write(
            estado
                .ruta_db
                .with_file_name(crate::ARCHIVO_RESTAURACION_FALLIDA),
            format!("No se pudo restaurar el respaldo: {e}"),
        );
    }
    app.restart();
}
