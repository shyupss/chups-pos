// Reglas de las actualizaciones: una búsqueda al día y nunca con un turno abierto.

use pos_core::servicios::actualizaciones::{self as act, INTERVALO_BUSQUEDA};
use pos_core::servicios::seguridad::ControlPin;
use pos_core::servicios::turnos;
use pos_core::{db, Error};

const T0: i64 = 1_798_804_800;

#[tokio::test]
async fn la_primera_vez_toca_buscar_y_despues_solo_pasado_un_dia() {
    let pool = db::abrir_en_memoria().await.unwrap();
    assert!(act::toca_buscar(&pool, T0).await.unwrap());
    act::registrar_busqueda(&pool, T0).await.unwrap();
    assert_eq!(act::ultima_busqueda(&pool).await.unwrap(), Some(T0));
    // Una hora después, 23 horas después: no.
    assert!(!act::toca_buscar(&pool, T0 + 3600).await.unwrap());
    assert!(!act::toca_buscar(&pool, T0 + INTERVALO_BUSQUEDA - 1)
        .await
        .unwrap());
    // Pasado un día: sí.
    assert!(act::toca_buscar(&pool, T0 + INTERVALO_BUSQUEDA)
        .await
        .unwrap());
}

#[tokio::test]
async fn una_busqueda_que_no_llego_al_servidor_no_cuenta() {
    // El controlador solo registra la búsqueda si hubo respuesta: sin registro, se reintenta.
    let pool = db::abrir_en_memoria().await.unwrap();
    assert!(act::toca_buscar(&pool, T0).await.unwrap());
    assert!(act::toca_buscar(&pool, T0 + 60).await.unwrap());
}

#[tokio::test]
async fn con_un_turno_abierto_no_se_busca_ni_se_instala() {
    let pool = db::abrir_en_memoria().await.unwrap();
    turnos::abrir(&pool, 0).await.unwrap();
    assert!(!act::toca_buscar(&pool, T0).await.unwrap());
    assert!(matches!(
        act::exigir_sin_turno(&pool).await,
        Err(Error::Validacion(m)) if m.contains("Cierra el turno")
    ));
    // Cerrado el turno, se puede.
    turnos::cerrar(&pool, &ControlPin::new(), 0, None, None, None)
        .await
        .unwrap();
    assert!(act::toca_buscar(&pool, T0).await.unwrap());
    assert!(act::exigir_sin_turno(&pool).await.is_ok());
}
