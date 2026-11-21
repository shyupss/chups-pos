//! Arranque de la app y controlador (la "C" de MVC).
//!
//! Aquí solo se conecta todo: se abre la base de datos, se guarda el estado
//! compartido y se registran los comandos que la interfaz puede llamar.
//! La lógica vive en `pos-core`.

mod comandos;
mod estado;

use std::time::Instant;

use tauri::Manager;

use estado::EstadoApp;

const NOMBRE_ARCHIVO_DB: &str = "chups-pos.db";

pub fn run() {
    let inicio = Instant::now();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let carpeta = app.path().app_data_dir()?;
            std::fs::create_dir_all(&carpeta)?;
            let ruta_db = carpeta.join(NOMBRE_ARCHIVO_DB);

            let pool = tauri::async_runtime::block_on(pos_core::db::abrir(&ruta_db))?;

            app.manage(EstadoApp {
                pool,
                ruta_db,
                ms_inicio: inicio.elapsed().as_millis() as u64,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            comandos::sistema::estado_sistema,
            comandos::productos::buscar_producto_por_codigo,
            comandos::productos::buscar_productos,
            comandos::productos::crear_producto,
            comandos::productos::editar_producto,
            comandos::productos::cambiar_estado_producto,
            comandos::ventas::confirmar_venta,
            comandos::config::config_caja,
            comandos::config::cambiar_redondeo_efectivo,
            comandos::categorias::listar_categorias,
            comandos::categorias::crear_categoria,
            comandos::categorias::renombrar_categoria,
            comandos::categorias::eliminar_categoria,
            comandos::espera::poner_en_espera,
            comandos::espera::listar_en_espera,
            comandos::espera::recuperar_en_espera,
            comandos::espera::descartar_en_espera,
            comandos::negocio::obtener_negocio,
            comandos::negocio::guardar_negocio,
            comandos::negocio::elegir_logo,
            comandos::negocio::quitar_logo,
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar CHUPS POS");
}
