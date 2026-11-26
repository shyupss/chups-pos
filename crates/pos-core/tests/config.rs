// Configuración tipada: el tamaño de la interfaz (escala) y su excepción del PIN.

use pos_core::servicios::config::{self, Config, EscalaInterfaz};
use pos_core::servicios::seguridad::{self, ControlPin};
use pos_core::{db, Error, FalloPin};

/// Lo que haría la pantalla: la configuración que hay con un solo cambio.
async fn con_escala(pool: &sqlx::SqlitePool, escala: EscalaInterfaz) -> Config {
    Config {
        escala_interfaz: escala,
        ..config::obtener(pool).await.unwrap()
    }
}

#[tokio::test]
async fn la_escala_por_defecto_es_grande() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let c = config::obtener(&pool).await.unwrap();
    assert_eq!(c.escala_interfaz, EscalaInterfaz::Grande);
    assert_eq!(Config::default().escala_interfaz, EscalaInterfaz::Grande);
    assert_eq!(c.escala_interfaz.factor(), 1.10);
}

#[test]
fn cada_escala_tiene_su_zoom() {
    assert_eq!(EscalaInterfaz::Normal.factor(), 1.0);
    assert_eq!(EscalaInterfaz::Grande.factor(), 1.10);
    assert_eq!(EscalaInterfaz::MasGrande.factor(), 1.25);
}

#[tokio::test]
async fn la_escala_se_guarda_y_se_lee() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let control = ControlPin::new();
    for escala in [
        EscalaInterfaz::MasGrande,
        EscalaInterfaz::Normal,
        EscalaInterfaz::Grande,
    ] {
        let guardada = config::guardar(&pool, &control, None, &con_escala(&pool, escala).await)
            .await
            .unwrap();
        assert_eq!(guardada.escala_interfaz, escala);
        assert_eq!(
            config::obtener(&pool).await.unwrap().escala_interfaz,
            escala
        );
    }
}

#[tokio::test]
async fn la_escala_viaja_como_texto_para_la_vista() {
    let json = serde_json::to_value(Config {
        escala_interfaz: EscalaInterfaz::MasGrande,
        ..Config::default()
    })
    .unwrap();
    assert_eq!(json["escalaInterfaz"], "masgrande");
    let c: Config = serde_json::from_value(json).unwrap();
    assert_eq!(c.escala_interfaz, EscalaInterfaz::MasGrande);
}

#[tokio::test]
async fn un_valor_raro_en_la_tabla_vale_el_defecto() {
    let pool = db::abrir_en_memoria().await.unwrap();
    sqlx::query("INSERT INTO config (clave, valor) VALUES ('escala_interfaz', 'enorme')")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        config::obtener(&pool).await.unwrap().escala_interfaz,
        EscalaInterfaz::Grande
    );
}

#[tokio::test]
async fn cambiar_solo_la_escala_no_pide_el_pin() {
    let pool = db::abrir_en_memoria().await.unwrap();
    seguridad::crear_pin(&pool, "4821").await.unwrap();
    let control = ControlPin::new();
    // "Cambiar configuración" está protegida por defecto: cambiar otra cosa sí lo pide.
    let otra_cosa = Config {
        copias: 2,
        ..config::obtener(&pool).await.unwrap()
    };
    assert!(matches!(
        config::guardar(&pool, &control, None, &otra_cosa).await,
        Err(Error::Pin(FalloPin::Requerido))
    ));

    // El tamaño de la interfaz, solo, no.
    let guardada = config::guardar(
        &pool,
        &control,
        None,
        &con_escala(&pool, EscalaInterfaz::MasGrande).await,
    )
    .await
    .unwrap();
    assert_eq!(guardada.escala_interfaz, EscalaInterfaz::MasGrande);
    assert_eq!(guardada.copias, 1);
}

#[tokio::test]
async fn cambiar_la_escala_junto_con_otro_ajuste_si_pide_el_pin() {
    let pool = db::abrir_en_memoria().await.unwrap();
    seguridad::crear_pin(&pool, "4821").await.unwrap();
    let control = ControlPin::new();
    let mezcla = Config {
        escala_interfaz: EscalaInterfaz::Normal,
        copias: 2,
        ..config::obtener(&pool).await.unwrap()
    };
    assert!(matches!(
        config::guardar(&pool, &control, None, &mezcla).await,
        Err(Error::Pin(FalloPin::Requerido))
    ));
    // No se guardó nada: ni la copia ni la escala.
    let sigue = config::obtener(&pool).await.unwrap();
    assert_eq!(sigue.escala_interfaz, EscalaInterfaz::Grande);
    assert_eq!(sigue.copias, 1);

    // Con el PIN correcto sí.
    let guardada = config::guardar(&pool, &control, Some("4821"), &mezcla)
        .await
        .unwrap();
    assert_eq!(guardada.escala_interfaz, EscalaInterfaz::Normal);
    assert_eq!(guardada.copias, 2);
}
