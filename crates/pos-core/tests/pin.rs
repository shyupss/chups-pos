// PIN de administrador: creación, verificación, bloqueo, recuperación y acciones protegidas.

use std::time::Duration;

use pos_core::servicios::config::{self, AccionesProtegidas, Config};
use pos_core::servicios::negocio::{self, DatosNegocio};
use pos_core::servicios::seguridad::{self, Accion, ControlPin, MAX_INTENTOS};
use pos_core::{db, Error, FalloPin};
use sqlx::SqlitePool;

const TODAS: [Accion; 5] = [
    Accion::AnularVenta,
    Accion::Devolucion,
    Accion::RetiroEfectivo,
    Accion::CerrarCaja,
    Accion::CambiarConfiguracion,
];

fn fallo(r: Result<impl std::fmt::Debug, Error>) -> FalloPin {
    match r {
        Err(Error::Pin(f)) => f,
        otro => panic!("se esperaba un error de PIN y vino {otro:?}"),
    }
}

async fn con_pin(pin: &str) -> (SqlitePool, String) {
    let pool = db::abrir_en_memoria().await.unwrap();
    let codigo = seguridad::crear_pin(&pool, pin).await.unwrap();
    (pool, codigo)
}

#[tokio::test]
async fn sin_pin_nada_se_pide() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let control = ControlPin::new();
    let estado = seguridad::estado(&pool).await.unwrap();
    assert!(!estado.tiene_pin);
    assert_eq!(estado.protegidas, AccionesProtegidas::default());
    for accion in TODAS {
        seguridad::exigir(&pool, &control, accion, None)
            .await
            .unwrap();
        // Un PIN cualquiera tampoco molesta.
        seguridad::exigir(&pool, &control, accion, Some("0000"))
            .await
            .unwrap();
    }
    config::guardar(&pool, &control, None, &Config::default())
        .await
        .unwrap();
}

#[tokio::test]
async fn crea_y_verifica_el_pin_guardando_solo_el_hash() {
    let (pool, codigo) = con_pin("4821").await;
    let control = ControlPin::new();

    // "ABCDE-FGHJK": 10 caracteres de base32 de Crockford con un guion al medio.
    assert_eq!(codigo.len(), 11, "{codigo}");
    assert_eq!(&codigo[5..6], "-");
    assert!(seguridad::estado(&pool).await.unwrap().tiene_pin);

    let guardados: Vec<(String, String)> =
        sqlx::query_as("SELECT clave, valor FROM config WHERE clave LIKE 'pin%'")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(guardados.len(), 2);
    for (clave, valor) in &guardados {
        assert!(valor.starts_with("$argon2id$"), "{clave}: {valor}");
        assert!(!valor.contains("4821") && !valor.contains(&codigo.replace('-', "")));
    }

    seguridad::exigir(&pool, &control, Accion::AnularVenta, Some("4821"))
        .await
        .unwrap();
    assert_eq!(
        fallo(seguridad::exigir(&pool, &control, Accion::AnularVenta, Some("4822")).await),
        FalloPin::Incorrecto { quedan: 4 }
    );
    // Lo que no tiene forma de PIN también es incorrecto (y cuenta).
    assert_eq!(
        fallo(seguridad::exigir(&pool, &control, Accion::AnularVenta, Some("abc")).await),
        FalloPin::Incorrecto { quedan: 3 }
    );
    // Un acierto reinicia la cuenta.
    seguridad::exigir(&pool, &control, Accion::AnularVenta, Some("4821"))
        .await
        .unwrap();
    assert_eq!(
        fallo(seguridad::exigir(&pool, &control, Accion::AnularVenta, Some("1111")).await),
        FalloPin::Incorrecto { quedan: 4 }
    );

    // No se crea otro encima del que hay.
    assert!(matches!(
        seguridad::crear_pin(&pool, "1234").await,
        Err(Error::Conflicto(_))
    ));
}

#[tokio::test]
async fn el_pin_nuevo_debe_tener_4_a_6_digitos() {
    let pool = db::abrir_en_memoria().await.unwrap();
    for malo in ["123", "1234567", "12a4", "", "12 34"] {
        assert!(
            matches!(
                seguridad::crear_pin(&pool, malo).await,
                Err(Error::Validacion(_))
            ),
            "{malo:?}"
        );
    }
    seguridad::crear_pin(&pool, "123456").await.unwrap();
}

#[tokio::test]
async fn bloquea_tras_cinco_intentos_fallidos() {
    let (pool, _) = con_pin("4821").await;
    let control = ControlPin::con_bloqueo(Duration::from_millis(300));

    for quedan in (1..MAX_INTENTOS).rev() {
        assert_eq!(
            fallo(seguridad::exigir(&pool, &control, Accion::Devolucion, Some("0000")).await),
            FalloPin::Incorrecto { quedan }
        );
    }
    // El quinto fallo bloquea...
    assert!(matches!(
        fallo(seguridad::exigir(&pool, &control, Accion::Devolucion, Some("0000")).await),
        FalloPin::Bloqueado { .. }
    ));
    // ...y mientras dura, ni el PIN correcto pasa.
    assert!(matches!(
        fallo(seguridad::exigir(&pool, &control, Accion::Devolucion, Some("4821")).await),
        FalloPin::Bloqueado { .. }
    ));
    // Otra instancia de la app (otro control) no hereda el bloqueo.
    seguridad::exigir(&pool, &ControlPin::new(), Accion::Devolucion, Some("4821"))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(350)).await;
    seguridad::exigir(&pool, &control, Accion::Devolucion, Some("4821"))
        .await
        .unwrap();
    // Después del bloqueo vuelven los cinco intentos.
    assert_eq!(
        fallo(seguridad::exigir(&pool, &control, Accion::Devolucion, Some("0000")).await),
        FalloPin::Incorrecto { quedan: 4 }
    );
}

#[tokio::test]
async fn el_bloqueo_por_defecto_dura_treinta_segundos() {
    let (pool, _) = con_pin("4821").await;
    let control = ControlPin::new();
    for _ in 0..MAX_INTENTOS {
        let _ = seguridad::exigir(&pool, &control, Accion::Devolucion, Some("0000")).await;
    }
    let f = fallo(seguridad::exigir(&pool, &control, Accion::Devolucion, Some("4821")).await);
    assert!(
        matches!(f, FalloPin::Bloqueado { segundos } if (29..=30).contains(&segundos)),
        "{f:?}"
    );
    assert!(f.to_string().contains("segundos"), "{f}");
}

#[tokio::test]
async fn protege_solo_las_acciones_marcadas() {
    let (pool, _) = con_pin("4821").await;
    let control = ControlPin::new();

    // Por defecto: todo menos cerrar la caja.
    for accion in TODAS {
        let r = seguridad::exigir(&pool, &control, accion, None).await;
        if accion == Accion::CerrarCaja {
            r.unwrap();
        } else {
            assert_eq!(fallo(r), FalloPin::Requerido, "{accion:?}");
        }
    }

    // Cambiar las protegidas siempre pide el PIN, aunque "cambiar configuración" no esté marcada.
    let nuevas = AccionesProtegidas {
        anular_venta: false,
        devolucion: true,
        retiro_efectivo: true,
        cerrar_caja: true,
        cambiar_configuracion: false,
    };
    assert_eq!(
        fallo(seguridad::guardar_protegidas(&pool, &control, nuevas, None).await),
        FalloPin::Requerido
    );
    assert!(matches!(
        fallo(seguridad::guardar_protegidas(&pool, &control, nuevas, Some("1111")).await),
        FalloPin::Incorrecto { .. }
    ));
    let estado = seguridad::guardar_protegidas(&pool, &control, nuevas, Some("4821"))
        .await
        .unwrap();
    assert_eq!(estado.protegidas, nuevas);

    seguridad::exigir(&pool, &control, Accion::AnularVenta, None)
        .await
        .unwrap();
    assert_eq!(
        fallo(seguridad::exigir(&pool, &control, Accion::CerrarCaja, None).await),
        FalloPin::Requerido
    );
    // Configuración ya no protegida: se guarda sin PIN...
    config::guardar(&pool, &control, None, &Config::default())
        .await
        .unwrap();
    // ...pero volver a protegerla sí lo pide.
    assert_eq!(
        fallo(
            seguridad::guardar_protegidas(&pool, &control, AccionesProtegidas::default(), None)
                .await
        ),
        FalloPin::Requerido
    );
}

#[tokio::test]
async fn cambiar_la_configuracion_pide_el_pin() {
    let (pool, _) = con_pin("4821").await;
    let control = ControlPin::new();
    let c = Config {
        redondeo_efectivo: false,
        ..Config::default()
    };

    assert_eq!(
        fallo(config::guardar(&pool, &control, None, &c).await),
        FalloPin::Requerido
    );
    assert!(config::obtener(&pool).await.unwrap().redondeo_efectivo);
    let guardada = config::guardar(&pool, &control, Some("4821"), &c)
        .await
        .unwrap();
    assert!(!guardada.redondeo_efectivo);

    // Los datos del negocio y el logo también son configuración.
    let datos = DatosNegocio {
        nombre: "Almacén".into(),
        rut: None,
        direccion: None,
        telefono: None,
        mensaje_pie: String::new(),
    };
    assert_eq!(
        fallo(negocio::guardar(&pool, &control, None, &datos).await),
        FalloPin::Requerido
    );
    assert_eq!(
        fallo(negocio::quitar_logo(&pool, &control, None).await),
        FalloPin::Requerido
    );
    negocio::guardar(&pool, &control, Some("4821"), &datos)
        .await
        .unwrap();

    // Guardar la configuración no toca el PIN.
    assert!(seguridad::estado(&pool).await.unwrap().tiene_pin);
}

#[tokio::test]
async fn cambia_y_quita_el_pin_con_el_actual() {
    let (pool, _) = con_pin("4821").await;
    let control = ControlPin::new();

    assert!(matches!(
        fallo(seguridad::cambiar_pin(&pool, &control, "1111", "5555").await),
        FalloPin::Incorrecto { .. }
    ));
    seguridad::cambiar_pin(&pool, &control, "4821", "555555")
        .await
        .unwrap();
    assert!(matches!(
        fallo(seguridad::exigir(&pool, &control, Accion::Devolucion, Some("4821")).await),
        FalloPin::Incorrecto { .. }
    ));
    seguridad::exigir(&pool, &control, Accion::Devolucion, Some("555555"))
        .await
        .unwrap();

    seguridad::quitar_pin(&pool, &control, "555555")
        .await
        .unwrap();
    assert!(!seguridad::estado(&pool).await.unwrap().tiene_pin);
    seguridad::exigir(&pool, &control, Accion::Devolucion, None)
        .await
        .unwrap();
    // Sin PIN se puede crear otro.
    seguridad::crear_pin(&pool, "9876").await.unwrap();
}

#[tokio::test]
async fn el_codigo_de_recuperacion_pone_un_pin_nuevo_y_se_renueva() {
    let (pool, codigo) = con_pin("4821").await;
    let control = ControlPin::new();

    assert_eq!(
        fallo(seguridad::recuperar_pin(&pool, &control, "ZZZZZ-ZZZZZ", "2468").await),
        FalloPin::CodigoIncorrecto { quedan: 4 }
    );
    // Se acepta como se haya anotado: minúsculas y sin guion.
    let escrito = codigo.replace('-', " ").to_lowercase();
    let nuevo_codigo = seguridad::recuperar_pin(&pool, &control, &escrito, "2468")
        .await
        .unwrap();
    assert_ne!(nuevo_codigo, codigo);

    seguridad::exigir(&pool, &control, Accion::AnularVenta, Some("2468"))
        .await
        .unwrap();
    assert!(matches!(
        fallo(seguridad::exigir(&pool, &control, Accion::AnularVenta, Some("4821")).await),
        FalloPin::Incorrecto { .. }
    ));
    // El código usado ya no sirve; el nuevo sí.
    assert!(matches!(
        fallo(seguridad::recuperar_pin(&pool, &control, &codigo, "1357").await),
        FalloPin::CodigoIncorrecto { .. }
    ));
    seguridad::recuperar_pin(&pool, &control, &nuevo_codigo, "1357")
        .await
        .unwrap();
    seguridad::exigir(&pool, &control, Accion::AnularVenta, Some("1357"))
        .await
        .unwrap();
}

#[tokio::test]
async fn los_codigos_incorrectos_tambien_bloquean() {
    let (pool, codigo) = con_pin("4821").await;
    let control = ControlPin::new();
    for _ in 0..MAX_INTENTOS {
        let _ = seguridad::recuperar_pin(&pool, &control, "00000-00000", "2468").await;
    }
    assert!(matches!(
        fallo(seguridad::recuperar_pin(&pool, &control, &codigo, "2468").await),
        FalloPin::Bloqueado { .. }
    ));
    // El bloqueo es uno solo: tampoco deja probar PIN.
    assert!(matches!(
        fallo(seguridad::exigir(&pool, &control, Accion::AnularVenta, Some("4821")).await),
        FalloPin::Bloqueado { .. }
    ));
}
