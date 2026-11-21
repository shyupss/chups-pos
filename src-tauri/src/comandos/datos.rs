//! Importar y exportar Excel. Leer y escribir archivos va siempre fuera de los hilos async.

use std::path::PathBuf;

use chrono::Utc;
use pos_core::servicios::exportaciones::{self, Exportacion};
use pos_core::servicios::importacion::{
    self as srv, Apertura, CampoCliente, CampoProducto, ResultadoImportacion, Tipo, VistaClientes,
    VistaProductos,
};
use pos_core::servicios::reportes::Rango;
use pos_core::zona::{Zona, ZonaLocal};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::{a_texto, ResultadoComando, ResultadoProtegido};
use crate::estado::EstadoApp;

/// Documentos\CHUPS POS\<sub>, junto a la carpeta de las boletas.
fn carpeta(estado: &EstadoApp, sub: &str) -> PathBuf {
    let c = estado
        .carpeta_pdf
        .parent()
        .map(|p| p.join(sub))
        .unwrap_or_else(|| estado.carpeta_pdf.clone());
    let _ = std::fs::create_dir_all(&c);
    c
}

/// "Guardar como" para un .xlsx. `None` si se cancela.
async fn guardar_como(
    app: &AppHandle,
    directorio: PathBuf,
    nombre: String,
) -> ResultadoComando<Option<PathBuf>> {
    let app = app.clone();
    let elegida = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Guardar como")
            .set_directory(directorio)
            .set_file_name(format!("{nombre}.xlsx"))
            .add_filter("Excel", &["xlsx"])
            .blocking_save_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(elegida.and_then(|r| r.into_path().ok()))
}

/// Elige la planilla a importar. `None` si se cancela.
#[tauri::command]
pub async fn elegir_planilla(app: AppHandle) -> ResultadoComando<Option<String>> {
    let elegida = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Planilla para importar")
            .add_filter("Planillas", &["xlsx", "xlsm", "xls", "ods", "csv"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(elegida
        .and_then(|r| r.into_path().ok())
        .map(|r| r.to_string_lossy().into_owned()))
}

/// Lee la planilla y propone qué columna es qué campo.
#[tauri::command]
pub async fn abrir_planilla(ruta: String) -> ResultadoComando<Apertura> {
    tauri::async_runtime::spawn_blocking(move || srv::abrir(std::path::Path::new(&ruta)))
        .await
        .map_err(|e| e.to_string())?
        .map_err(a_texto)
}

#[tauri::command]
pub async fn previsualizar_productos(
    estado: State<'_, EstadoApp>,
    ruta: String,
    mapeo: Vec<Option<CampoProducto>>,
) -> ResultadoComando<VistaProductos> {
    srv::previsualizar_productos(&estado.pool, &ruta, &mapeo)
        .await
        .map_err(a_texto)
}

#[tauri::command]
pub async fn previsualizar_clientes(
    estado: State<'_, EstadoApp>,
    ruta: String,
    mapeo: Vec<Option<CampoCliente>>,
) -> ResultadoComando<VistaClientes> {
    srv::previsualizar_clientes(&estado.pool, &ruta, &mapeo)
        .await
        .map_err(a_texto)
}

/// Importa los productos válidos en una transacción. Pide el PIN de "importar datos".
#[tauri::command]
pub async fn importar_productos(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
    ruta: String,
    mapeo: Vec<Option<CampoProducto>>,
    huella: String,
    pin: Option<String>,
) -> ResultadoProtegido<ResultadoImportacion> {
    let r = srv::importar_productos(
        &estado.pool,
        &estado.pin,
        &ruta,
        &mapeo,
        &huella,
        pin.as_deref(),
    )
    .await?;
    super::avisar_cambios(&app);
    Ok(r)
}

/// Importa los clientes válidos (sin deudas). Pide el PIN de "importar datos".
#[tauri::command]
pub async fn importar_clientes(
    estado: State<'_, EstadoApp>,
    ruta: String,
    mapeo: Vec<Option<CampoCliente>>,
    huella: String,
    pin: Option<String>,
) -> ResultadoProtegido<ResultadoImportacion> {
    Ok(srv::importar_clientes(
        &estado.pool,
        &estado.pin,
        &ruta,
        &mapeo,
        &huella,
        pin.as_deref(),
    )
    .await?)
}

/// Guarda la planilla de ejemplo (encabezados y dos filas). `None` si se cancela.
#[tauri::command]
pub async fn descargar_planilla_ejemplo(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
    tipo: Tipo,
) -> ResultadoComando<Option<String>> {
    let nombre = match tipo {
        Tipo::Productos => "planilla-productos",
        Tipo::Clientes => "planilla-clientes",
    };
    let Some(ruta) = guardar_como(&app, carpeta(&estado, "Exportaciones"), nombre.into()).await?
    else {
        return Ok(None);
    };
    let destino = ruta.clone();
    tauri::async_runtime::spawn_blocking(move || exportaciones::planilla_ejemplo(tipo, &destino))
        .await
        .map_err(|e| e.to_string())?
        .map_err(a_texto)?;
    Ok(Some(ruta.to_string_lossy().into_owned()))
}

/// Exporta datos a Excel (ventas, movimientos y compras con su rango). `None` si se cancela.
#[tauri::command]
pub async fn exportar_datos(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
    que: Exportacion,
    desde: Option<String>,
    hasta: Option<String>,
) -> ResultadoComando<Option<String>> {
    let rango = match (desde, hasta) {
        (Some(d), Some(h)) => Some(Rango::de_textos(&d, &h).map_err(a_texto)?),
        _ => None,
    };
    let nombre = match (que.usa_rango(), rango) {
        (true, Some(r)) => pos_core::datos::nombre_archivo(que.nombre_base(), r.desde, r.hasta),
        _ => format!("{}-{}", que.nombre_base(), ZonaLocal.dia(Utc::now())),
    };
    let Some(ruta) = guardar_como(&app, carpeta(&estado, "Exportaciones"), nombre).await? else {
        return Ok(None);
    };
    exportaciones::exportar(
        &estado.pool,
        &ZonaLocal,
        que,
        rango,
        ruta.clone(),
        Utc::now(),
    )
    .await
    .map_err(a_texto)?;
    Ok(Some(ruta.to_string_lossy().into_owned()))
}
