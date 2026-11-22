// Primer arranque: la base vacía pide el asistente; una con datos, no; omitir pasos no rompe nada.

mod comun;

use pos_core::db;
use pos_core::servicios::negocio::{self, DatosNegocio};
use pos_core::servicios::primer_arranque;
use pos_core::servicios::seguridad::ControlPin;
use pos_core::servicios::turnos;

#[tokio::test]
async fn una_base_vacia_pide_el_asistente() {
    let pool = db::abrir_en_memoria().await.unwrap();
    assert!(primer_arranque::pendiente(&pool).await.unwrap());
}

#[tokio::test]
async fn una_base_con_datos_no_lo_pide() {
    // Un producto.
    let pool = db::abrir_en_memoria().await.unwrap();
    comun::crear(&pool, "780", "Pan", 1000, 5).await;
    assert!(!primer_arranque::pendiente(&pool).await.unwrap());

    // Solo un turno abierto.
    let pool = db::abrir_en_memoria().await.unwrap();
    turnos::abrir(&pool, 0).await.unwrap();
    assert!(!primer_arranque::pendiente(&pool).await.unwrap());
}

#[tokio::test]
async fn una_base_de_la_fase_6_abre_directo_a_la_caja() {
    // Datos de varias fases, sin la clave del asistente (no existía): no se ofrece.
    let dir = tempfile::tempdir().unwrap();
    let ruta = dir.path().join("chups-pos.db");
    {
        let pool = db::abrir(&ruta).await.unwrap();
        comun::crear(&pool, "780", "Pan", 1000, 5).await;
        sqlx::query("DELETE FROM config WHERE clave = 'asistente_terminado'")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
    }
    let pool = db::abrir(&ruta).await.unwrap();
    assert!(!primer_arranque::pendiente(&pool).await.unwrap());
}

#[tokio::test]
async fn terminar_lo_apaga_aunque_se_hayan_omitido_todos_los_pasos() {
    let pool = db::abrir_en_memoria().await.unwrap();
    assert!(primer_arranque::pendiente(&pool).await.unwrap());
    // Se omitió todo: el negocio sigue sin nombre y no hay PIN ni impresora.
    primer_arranque::terminar(&pool).await.unwrap();
    assert!(!primer_arranque::pendiente(&pool).await.unwrap());
    assert_eq!(negocio::obtener(&pool).await.unwrap().nombre, "");
    // Terminar dos veces (se volvió a abrir desde Configuración) no molesta.
    primer_arranque::terminar(&pool).await.unwrap();
    assert!(!primer_arranque::pendiente(&pool).await.unwrap());
}

#[tokio::test]
async fn guardar_el_negocio_no_lo_da_por_terminado() {
    // Si se cierra la app después del paso 1, el asistente sigue pendiente al volver.
    let pool = db::abrir_en_memoria().await.unwrap();
    negocio::guardar(
        &pool,
        &ControlPin::new(),
        None,
        &DatosNegocio {
            nombre: "Almacén Don Pepe".into(),
            rut: None,
            direccion: None,
            telefono: None,
            mensaje_pie: "Gracias".into(),
        },
    )
    .await
    .unwrap();
    assert!(primer_arranque::pendiente(&pool).await.unwrap());
}
