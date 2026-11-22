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
/// Inicio de la prueba, aparte de la base: reinstalar o borrar la base no la reinicia.
const ARCHIVO_PRUEBA: &str = "prueba.dat";
/// Cómo contactar al vendedor, para Configuración > Licencia. Se fija al compilar con la
/// variable de entorno CHUPS_CONTACTO_VENDEDOR.
const CONTACTO_VENDEDOR: &str = match option_env!("CHUPS_CONTACTO_VENDEDOR") {
    Some(c) => c,
    None => {
        "Envía el código de solicitud a quien te vendió CHUPS POS y te responderá con la licencia."
    }
};
/// Si restaurar un respaldo falla después de cerrar la base, el motivo queda aquí para mostrarlo
/// al volver a abrir la app (la base anterior queda intacta).
pub(crate) const ARCHIVO_RESTAURACION_FALLIDA: &str = "restauracion-fallida.txt";
/// El respaldo automático al abrir espera un poco: el arranque y la primera venta van primero.
const ESPERA_RESPALDO_AL_ABRIR: std::time::Duration = std::time::Duration::from_secs(20);

pub fn run() {
    let inicio = Instant::now();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let carpeta = app.path().app_data_dir()?;
            std::fs::create_dir_all(&carpeta)?;
            let ruta_db = carpeta.join(NOMBRE_ARCHIVO_DB);

            let pool = tauri::async_runtime::block_on(pos_core::db::abrir(&ruta_db))?;
            let lectura = tauri::async_runtime::block_on(pos_core::db::abrir_lectura(&ruta_db))?;
            let aviso = carpeta.join(ARCHIVO_RESTAURACION_FALLIDA);
            let restauracion_fallida = std::fs::read_to_string(&aviso).ok();
            let _ = std::fs::remove_file(&aviso);

            // Documentos del usuario; si Windows no la informa, la carpeta de datos.
            let carpeta_pdf = app
                .path()
                .document_dir()
                .unwrap_or_else(|_| carpeta.clone())
                .join("CHUPS POS")
                .join("Boletas");

            // Huella del equipo para la licencia. Si Windows no deja leer su identificador, se usa
            // solo el nombre del equipo: la licencia se puede atar igual.
            let huella = pos_licencia::huella_equipo().unwrap_or_else(|_| {
                pos_licencia::Huella::de("", &std::env::var("COMPUTERNAME").unwrap_or_default())
            });
            let licencia = pos_core::servicios::licencia::Contexto {
                clave_publica: pos_licencia::CLAVE_PUBLICA,
                huella,
                archivo_prueba: Some(carpeta.join(ARCHIVO_PRUEBA)),
                version: env!("CARGO_PKG_VERSION").to_owned(),
                contacto: CONTACTO_VENDEDOR.to_owned(),
            };
            // Fija el inicio de la prueba (primer arranque) y anota el instante para el reloj.
            tauri::async_runtime::block_on(pos_core::servicios::licencia::iniciar(
                &pool,
                &licencia,
                pos_core::servicios::licencia::ahora(),
            ))?;

            app.manage(EstadoApp {
                pool,
                lectura,
                ruta_db,
                ms_inicio: inicio.elapsed().as_millis() as u64,
                carpeta_pdf,
                hardware: pos_hardware::Hardware::new(&carpeta),
                pin: pos_core::servicios::seguridad::ControlPin::new(),
                carpeta_respaldos: carpeta.join("Respaldos"),
                error_respaldo: std::sync::Mutex::new(restauracion_fallida),
                licencia,
            });

            // Respaldo automático si el último tiene más de 24 horas (en segundo plano).
            let app_respaldo = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(ESPERA_RESPALDO_AL_ABRIR).await;
                comandos::respaldos::automatico(&app_respaldo, false);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            comandos::sistema::estado_sistema,
            comandos::productos::buscar_producto_por_codigo,
            comandos::productos::buscar_productos,
            comandos::productos::leer_codigo_balanza,
            comandos::productos::crear_producto,
            comandos::productos::editar_producto,
            comandos::productos::cambiar_estado_producto,
            comandos::ventas::confirmar_venta,
            comandos::ventas::listar_ventas,
            comandos::ventas::detalle_venta,
            comandos::ventas::venta_por_folio,
            comandos::ventas::anular_venta,
            comandos::ventas::imprimir_anulacion,
            comandos::ventas::registrar_devolucion,
            comandos::ventas::imprimir_devolucion,
            comandos::config::obtener_config,
            comandos::config::guardar_config,
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
            comandos::boletas::guardar_boleta_pdf,
            comandos::boletas::carpeta_pdf_predeterminada,
            comandos::boletas::elegir_carpeta_pdf,
            comandos::boletas::ultima_venta,
            comandos::impresion::listar_impresoras,
            comandos::impresion::estado_impresora,
            comandos::impresion::imprimir_boleta,
            comandos::impresion::imprimir_prueba,
            comandos::impresion::abrir_cajon,
            comandos::impresion::carpeta_impresora_pruebas,
            comandos::seguridad::estado_seguridad,
            comandos::seguridad::crear_pin,
            comandos::seguridad::cambiar_pin,
            comandos::seguridad::quitar_pin,
            comandos::seguridad::recuperar_pin,
            comandos::seguridad::guardar_protegidas,
            comandos::turnos::estado_caja,
            comandos::turnos::abrir_turno,
            comandos::turnos::registrar_movimiento_caja,
            comandos::turnos::resumen_turno,
            comandos::turnos::cerrar_turno,
            comandos::turnos::listar_turnos,
            comandos::turnos::imprimir_cierre,
            comandos::turnos::guardar_cierre_pdf,
            comandos::inventario::listar_stock,
            comandos::inventario::resumen_inventario,
            comandos::inventario::contar_bajo_minimo,
            comandos::inventario::listar_movimientos,
            comandos::inventario::previsualizar_conteo,
            comandos::inventario::ajustar_conteo,
            comandos::inventario::registrar_merma,
            comandos::compras::listar_proveedores,
            comandos::compras::crear_proveedor,
            comandos::compras::editar_proveedor,
            comandos::compras::cambiar_estado_proveedor,
            comandos::compras::registrar_compra,
            comandos::compras::detalle_compra,
            comandos::compras::listar_compras,
            comandos::compras::anular_compra,
            comandos::clientes::buscar_clientes,
            comandos::clientes::obtener_cliente,
            comandos::clientes::crear_cliente,
            comandos::clientes::editar_cliente,
            comandos::clientes::cambiar_estado_cliente,
            comandos::reportes::reporte_hoy,
            comandos::reportes::reporte_ventas,
            comandos::reportes::reporte_productos,
            comandos::reportes::reporte_ganancia,
            comandos::reportes::reporte_turnos,
            comandos::reportes::reporte_fiados,
            comandos::reportes::exportar_reporte,
            comandos::reportes::abrir_archivo,
            comandos::datos::elegir_planilla,
            comandos::datos::abrir_planilla,
            comandos::datos::previsualizar_productos,
            comandos::datos::previsualizar_clientes,
            comandos::datos::importar_productos,
            comandos::datos::importar_clientes,
            comandos::datos::descargar_planilla_ejemplo,
            comandos::datos::exportar_datos,
            comandos::respaldos::estado_respaldos,
            comandos::respaldos::respaldar_ahora,
            comandos::respaldos::respaldar_en,
            comandos::respaldos::elegir_carpeta_respaldos,
            comandos::respaldos::abrir_carpeta_respaldos,
            comandos::respaldos::elegir_respaldo,
            comandos::respaldos::inspeccionar_respaldo,
            comandos::respaldos::restaurar_respaldo,
            comandos::fiados::listar_deudores,
            comandos::fiados::ficha_cliente,
            comandos::fiados::registrar_pago,
            comandos::fiados::anular_pago,
            comandos::fiados::imprimir_pago,
            comandos::fiados::imprimir_estado_cuenta,
            comandos::fiados::guardar_estado_cuenta_pdf,
            comandos::licencia::estado_licencia,
            comandos::licencia::activar_licencia,
            comandos::licencia::cargar_archivo_licencia,
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar CHUPS POS");
}
