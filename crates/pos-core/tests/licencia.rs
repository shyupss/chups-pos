// Estados de la licencia: prueba, activa (permanente), gracia, modo consulta y reloj que retrocede.

mod comun;

use chrono::{DateTime, Utc};
use pos_core::servicios::clientes::{self, DatosCliente};
use pos_core::servicios::fiados::{self, NuevoPagoCliente};
use pos_core::servicios::licencia::{self, Contexto, Modo};
use pos_core::servicios::respaldos;
use pos_core::servicios::seguridad::ControlPin;
use pos_core::servicios::turnos;
use pos_core::servicios::ventas::{self, ItemPedido, MedioPago, NuevaVenta};
use pos_core::zona::ZonaLocal;
use pos_core::{db, Error};
use pos_licencia::{clave_publica_de, firmar, generar_clave_privada, Huella, Licencia};
use sqlx::SqlitePool;

const DIA: i64 = 86_400;
/// 1 de enero de 2027, 12:00 UTC: el primer arranque de los tests.
const T0: i64 = 1_798_804_800;

struct Vendedor {
    privada: [u8; 32],
    publica: [u8; 32],
}

fn vendedor() -> Vendedor {
    let privada = generar_clave_privada().unwrap();
    Vendedor {
        privada,
        publica: clave_publica_de(&privada),
    }
}

fn equipo(nombre: &str) -> Huella {
    Huella::de("6F9619FF-8B86-D011-B42D-00C04FC964FF", nombre)
}

fn ctx(v: &Vendedor, nombre_equipo: &str) -> Contexto {
    Contexto {
        clave_publica: v.publica,
        huella: equipo(nombre_equipo),
        archivo_prueba: None,
        version: "1.0.0".into(),
        contacto: "vendedor@ejemplo.cl".into(),
    }
}

fn emitir(v: &Vendedor, equipo: Option<Huella>) -> String {
    firmar(
        &Licencia::nueva("Almacén Don Pepe", Some("76.123.456-7"), 1, equipo).unwrap(),
        &v.privada,
    )
    .unwrap()
}

fn venta(producto_id: i64, medio: MedioPago, cliente_id: Option<i64>) -> NuevaVenta {
    NuevaVenta {
        items: vec![ItemPedido {
            producto_id,
            cantidad_mil: 1000,
        }],
        medio_pago: medio,
        monto_recibido: None,
        descuento: 0,
        cliente_id,
    }
}

async fn base() -> SqlitePool {
    db::abrir_en_memoria().await.unwrap()
}

fn es_licencia(r: Result<(), Error>) -> bool {
    matches!(r, Err(Error::Licencia(_)))
}

#[tokio::test]
async fn el_primer_arranque_empieza_la_prueba_de_15_dias() {
    let pool = base().await;
    let v = vendedor();
    let c = ctx(&v, "caja-1");
    let e = licencia::iniciar(&pool, &c, T0).await.unwrap();
    assert_eq!(e.modo, Modo::Prueba);
    assert_eq!(e.dias_restantes, Some(15));
    assert_eq!(e.dias_prueba, 15);
    assert!(e.puede_vender);
    assert!(e.licencia.is_none());
    assert!(!e.codigo_solicitud.is_empty());

    // Abrir de nuevo no reinicia la prueba.
    let e = licencia::iniciar(&pool, &c, T0 + 10 * DIA).await.unwrap();
    assert_eq!(e.dias_restantes, Some(5));
    let e = licencia::estado(&pool, &c, T0 + 14 * DIA + 3600)
        .await
        .unwrap();
    assert_eq!((e.modo, e.dias_restantes), (Modo::Prueba, Some(1)));
    assert!(licencia::exigir_abrir_turno(&pool, &c, T0 + 14 * DIA)
        .await
        .is_ok());
    assert!(licencia::exigir_venta(&pool, &c, T0 + 14 * DIA)
        .await
        .is_ok());
}

#[tokio::test]
async fn dias_prueba_sale_de_la_configuracion_pero_acotado() {
    let pool = base().await;
    let c = ctx(&vendedor(), "caja-1");
    sqlx::query("INSERT INTO config (clave, valor) VALUES ('dias_prueba', '30')")
        .execute(&pool)
        .await
        .unwrap();
    let e = licencia::iniciar(&pool, &c, T0).await.unwrap();
    assert_eq!(e.dias_restantes, Some(30));
    sqlx::query("UPDATE config SET valor = '99999' WHERE clave = 'dias_prueba'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        licencia::estado(&pool, &c, T0).await.unwrap().dias_prueba,
        60
    );
}

#[tokio::test]
async fn despues_de_la_prueba_hay_7_dias_de_gracia_con_todo() {
    let pool = comun::con_caja().await;
    let c = ctx(&vendedor(), "caja-1");
    licencia::iniciar(&pool, &c, T0).await.unwrap();
    let e = licencia::estado(&pool, &c, T0 + 16 * DIA).await.unwrap();
    assert_eq!((e.modo, e.dias_restantes), (Modo::Gracia, Some(6)));
    assert!(e.puede_vender);
    assert!(licencia::exigir_venta(&pool, &c, T0 + 16 * DIA)
        .await
        .is_ok());
    assert!(licencia::exigir_abrir_turno(&pool, &c, T0 + 21 * DIA)
        .await
        .is_ok());
}

#[tokio::test]
async fn modo_consulta_no_abre_turno_ni_vende_pero_permite_lo_demas() {
    let dir = tempfile::tempdir().unwrap();
    let pool = db::abrir(&dir.path().join("chups-pos.db")).await.unwrap();
    let c = ctx(&vendedor(), "caja-1");
    licencia::iniciar(&pool, &c, T0).await.unwrap();

    // Un fiado y un turno abiertos durante la prueba (los turnos usan el reloj real).
    let p = comun::crear(&pool, "780", "Pan", 1000, 50).await;
    let cliente = clientes::crear(
        &pool,
        &DatosCliente {
            nombre: "Rosa".into(),
            telefono: None,
            rut: None,
            direccion: None,
            nota: None,
            limite_credito: None,
        },
    )
    .await
    .unwrap()
    .id;
    turnos::abrir(&pool, 0).await.unwrap();
    ventas::confirmar(&pool, &venta(p.id, MedioPago::Fiado, Some(cliente)))
        .await
        .unwrap();

    // Un año después: modo consulta. El turno que quedó abierto ya pasó su margen.
    let despues = Utc::now().timestamp() + 400 * DIA;
    let e = licencia::estado(&pool, &c, despues).await.unwrap();
    assert_eq!(e.modo, Modo::Consulta);
    assert!(!e.puede_vender);
    assert!(es_licencia(
        licencia::exigir_venta(&pool, &c, despues).await
    ));
    assert!(es_licencia(
        licencia::exigir_abrir_turno(&pool, &c, despues).await
    ));

    // Cobrar el fiado, respaldar y cerrar el turno no dependen de la licencia.
    fiados::registrar_pago(
        &pool,
        &NuevoPagoCliente {
            cliente_id: cliente,
            monto: 1000,
            medio_pago: MedioPago::Efectivo,
            ventas: vec![],
            monto_recibido: None,
            nota: None,
        },
    )
    .await
    .unwrap();
    respaldos::respaldar(&pool, &dir.path().join("r"), &ZonaLocal, Utc::now(), None)
        .await
        .unwrap();
    turnos::cerrar(&pool, &ControlPin::new(), 1000, None, None, None)
        .await
        .unwrap();
    // Y con la caja cerrada, sigue sin poder abrirla.
    assert!(es_licencia(
        licencia::exigir_abrir_turno(&pool, &c, despues).await
    ));
}

#[tokio::test]
async fn la_venta_en_curso_no_se_corta_al_terminar_la_gracia() {
    let pool = comun::con_caja().await; // turno abierto ahora (reloj real)
    let c = ctx(&vendedor(), "caja-1");
    // Una prueba que empezó hace 22 días: la gracia termina justo ahora (± segundos).
    let ahora = Utc::now().timestamp();
    let inicio = ahora - 22 * DIA + 60;
    sqlx::query("INSERT INTO config (clave, valor) VALUES ('prueba_inicio', ?)")
        .bind(inicio.to_string())
        .execute(&pool)
        .await
        .unwrap();
    let fin_gracia = inicio + 22 * DIA;
    // Recién terminada la gracia: el turno que estaba abierto sigue vendiendo...
    let e = licencia::estado(&pool, &c, fin_gracia + 3600)
        .await
        .unwrap();
    assert_eq!(e.modo, Modo::Consulta);
    assert!(licencia::exigir_venta(&pool, &c, fin_gracia + 3600)
        .await
        .is_ok());
    // ...pero no puede abrir otro, y pasado el margen tampoco vende.
    assert!(es_licencia(
        licencia::exigir_abrir_turno(&pool, &c, fin_gracia + 3600).await
    ));
    assert!(es_licencia(
        licencia::exigir_venta(&pool, &c, fin_gracia + 25 * 3600).await
    ));
}

#[tokio::test]
async fn una_licencia_activa_es_permanente() {
    let pool = base().await;
    let v = vendedor();
    let c = ctx(&v, "caja-1");
    licencia::iniciar(&pool, &c, T0).await.unwrap();
    let e = licencia::activar(&pool, &c, &emitir(&v, None), T0 + DIA)
        .await
        .unwrap();
    assert_eq!(e.modo, Modo::Activa);
    assert_eq!(e.dias_restantes, None);
    let datos = e.licencia.unwrap();
    assert_eq!(datos.negocio, "Almacén Don Pepe");
    assert_eq!(datos.rut.as_deref(), Some("76.123.456-7"));
    assert!(datos.activada_en.unwrap().starts_with("2027-01-02"));

    // Con el reloj 50 años adelante, sigue activa y vende.
    let lejos = T0 + 50 * 365 * DIA;
    licencia::registrar_instante(&pool, lejos).await.unwrap();
    let e = licencia::estado(&pool, &c, lejos).await.unwrap();
    assert_eq!(e.modo, Modo::Activa);
    assert!(licencia::exigir_venta(&pool, &c, lejos).await.is_ok());
    assert!(licencia::exigir_abrir_turno(&pool, &c, lejos).await.is_ok());

    // Tras actualizar la app (otra versión), sigue activa.
    let mut nueva_version = ctx(&v, "caja-1");
    nueva_version.version = "2.3.0".into();
    let e = licencia::iniciar(&pool, &nueva_version, lejos + DIA)
        .await
        .unwrap();
    assert_eq!(e.modo, Modo::Activa);
}

#[tokio::test]
async fn sin_equipo_se_ata_al_primero_y_una_base_copiada_no_sirve_en_otro() {
    let pool = base().await;
    let v = vendedor();
    let c1 = ctx(&v, "caja-1");
    licencia::iniciar(&pool, &c1, T0).await.unwrap();
    let texto = emitir(&v, None);
    licencia::activar(&pool, &c1, &texto, T0).await.unwrap();

    // La misma base en otro computador: la licencia no vale ahí y se avisa; sigue la prueba.
    let c2 = ctx(&v, "caja-2");
    let e = licencia::estado(&pool, &c2, T0 + DIA).await.unwrap();
    assert_eq!(e.modo, Modo::Prueba);
    assert!(e.error_licencia.unwrap().contains("otro equipo"));
    // Volver a activarla ahí tampoco sirve.
    assert!(matches!(
        licencia::activar(&pool, &c2, &texto, T0 + DIA).await,
        Err(Error::Licencia(m)) if m.contains("otro equipo")
    ));
    // Con una licencia nueva para ese equipo, sí.
    let nueva = emitir(&v, Some(equipo("caja-2")));
    let e = licencia::activar(&pool, &c2, &nueva, T0 + DIA)
        .await
        .unwrap();
    assert_eq!(e.modo, Modo::Activa);
}

#[tokio::test]
async fn licencias_invalidas_se_rechazan_con_un_mensaje_claro() {
    let pool = base().await;
    let v = vendedor();
    let c = ctx(&v, "caja-1");
    licencia::iniciar(&pool, &c, T0).await.unwrap();

    // De otro equipo.
    let ajena = emitir(&v, Some(equipo("caja-9")));
    assert!(matches!(
        licencia::activar(&pool, &c, &ajena, T0).await,
        Err(Error::Licencia(m)) if m.contains("otro equipo")
    ));
    // Firmada por otra clave.
    let otra = emitir(&vendedor(), None);
    assert!(matches!(
        licencia::activar(&pool, &c, &otra, T0).await,
        Err(Error::Licencia(m)) if m.contains("firma")
    ));
    // Alterada.
    let mut b: Vec<char> = emitir(&v, None).chars().collect();
    // Un carácter del medio (nunca un guion ni los bits de relleno del final): siempre cambia datos firmados.
    let mut i = b.len() / 2;
    while b[i] == '-' {
        i += 1;
    }
    b[i] = if b[i] == 'A' { 'B' } else { 'A' };
    let alterada: String = b.into_iter().collect();
    assert!(matches!(
        licencia::activar(&pool, &c, &alterada, T0).await,
        Err(Error::Licencia(_))
    ));
    // Nada de eso quedó guardado.
    assert_eq!(
        licencia::estado(&pool, &c, T0).await.unwrap().modo,
        Modo::Prueba
    );
}

#[tokio::test]
async fn el_reloj_que_retrocede_no_devuelve_dias_y_se_avisa() {
    let pool = base().await;
    let c = ctx(&vendedor(), "caja-1");
    licencia::iniciar(&pool, &c, T0).await.unwrap();
    // Se vendió el día 10...
    licencia::registrar_instante(&pool, T0 + 10 * DIA)
        .await
        .unwrap();
    // ...y alguien atrasó el reloj al día 2: siguen quedando 5 días, no 13, y se avisa.
    let e = licencia::estado(&pool, &c, T0 + 2 * DIA).await.unwrap();
    assert_eq!(e.dias_restantes, Some(5));
    assert!(e.reloj_atrasado);
    // Un atraso menor a 48 horas (un ajuste de hora) no se avisa, pero tampoco da días.
    let e = licencia::estado(&pool, &c, T0 + 9 * DIA).await.unwrap();
    assert!(!e.reloj_atrasado);
    assert_eq!(e.dias_restantes, Some(5));
    // Atrasar el reloj después de la prueba no la reabre.
    licencia::registrar_instante(&pool, T0 + 30 * DIA)
        .await
        .unwrap();
    let e = licencia::estado(&pool, &c, T0 + DIA).await.unwrap();
    assert_eq!(e.modo, Modo::Consulta);
    assert!(es_licencia(
        licencia::exigir_abrir_turno(&pool, &c, T0 + DIA).await
    ));
}

#[tokio::test]
async fn reinstalar_con_una_base_nueva_no_reinicia_la_prueba() {
    let dir = tempfile::tempdir().unwrap();
    let v = vendedor();
    let mut c = ctx(&v, "caja-1");
    c.archivo_prueba = Some(dir.path().join("prueba.dat"));

    let primera = base().await;
    licencia::iniciar(&primera, &c, T0).await.unwrap();
    assert!(dir.path().join("prueba.dat").exists());

    // Base nueva (se borró la anterior) 20 días después: la prueba sigue contando desde T0.
    let nueva = base().await;
    let e = licencia::iniciar(&nueva, &c, T0 + 20 * DIA).await.unwrap();
    assert_eq!(e.modo, Modo::Gracia);
    let inicio: DateTime<Utc> = e.inicio_prueba.parse().unwrap();
    assert_eq!(inicio.timestamp(), T0);

    // Un archivo editado a mano o de otro equipo se ignora.
    std::fs::write(dir.path().join("prueba.dat"), "v1:1999999999:abc").unwrap();
    let otra = base().await;
    let e = licencia::iniciar(&otra, &c, T0 + 20 * DIA).await.unwrap();
    assert_eq!(e.modo, Modo::Prueba);
}

#[tokio::test]
async fn una_base_de_la_fase_6_entra_en_prueba_y_conserva_sus_datos() {
    let dir = tempfile::tempdir().unwrap();
    let ruta = dir.path().join("chups-pos.db");
    // Una base con datos, como la deja la fase 6 (sin claves de licencia).
    {
        let pool = db::abrir(&ruta).await.unwrap();
        comun::crear(&pool, "780", "Pan", 1000, 5).await;
        sqlx::query("DELETE FROM config WHERE clave LIKE 'licencia%' OR clave LIKE 'prueba%'")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
    }
    let pool = db::abrir(&ruta).await.unwrap();
    let c = ctx(&vendedor(), "caja-1");
    let e = licencia::iniciar(&pool, &c, T0).await.unwrap();
    assert_eq!(e.modo, Modo::Prueba);
    let productos: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM productos")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(productos, 1);
}
