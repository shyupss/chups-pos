// Respaldos: VACUUM INTO con la base abierta y vendiendo, rotación de los últimos N, rechazo de
// un archivo ajeno y de un esquema más nuevo, y restaurar un respaldo antiguo que luego migra.

mod comun;

use std::path::Path;

use chrono::{Duration, Utc};
use pos_core::servicios::respaldos;
use pos_core::servicios::ventas::{self, ItemPedido, MedioPago, NuevaVenta};
use pos_core::zona::ZonaLocal;
use pos_core::{db, Error};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

fn venta(producto_id: i64) -> NuevaVenta {
    NuevaVenta {
        items: vec![ItemPedido {
            producto_id,
            cantidad_mil: 1000,
        }],
        medio_pago: MedioPago::Debito,
        monto_recibido: None,
        descuento: 0,
        cliente_id: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn respalda_con_la_base_abierta_y_vendiendo() {
    let dir = tempfile::tempdir().unwrap();
    let pool = db::abrir(&dir.path().join("chups-pos.db")).await.unwrap();
    pos_core::servicios::turnos::abrir(&pool, 0).await.unwrap();
    let p = comun::crear(&pool, "A", "Leche", 1000, 100_000).await;
    for _ in 0..50 {
        ventas::confirmar(&pool, &venta(p.id)).await.unwrap();
    }
    // Mientras se respalda, la caja sigue vendiendo.
    let vendiendo = {
        let pool = pool.clone();
        tokio::spawn(async move {
            for _ in 0..200 {
                ventas::confirmar(&pool, &venta(p.id)).await.unwrap();
            }
        })
    };
    let carpeta = dir.path().join("Respaldos");
    let r = respaldos::respaldar(&pool, &carpeta, &ZonaLocal, Utc::now(), None)
        .await
        .unwrap();
    vendiendo.await.unwrap();
    assert!(
        r.nombre.starts_with("chups-respaldo-") && r.nombre.ends_with(".db"),
        "{}",
        r.nombre
    );
    assert!(r.bytes > 0);
    // Es una base sana de CHUPS con las ventas que había al momento de copiar.
    let info = respaldos::inspeccionar(Path::new(&r.ruta)).await.unwrap();
    assert_eq!(info.version_esquema, db::version_app());
    assert!(info.ventas >= 50 && info.ventas <= 250, "{}", info.ventas);
    assert_eq!(info.productos, 1);
    // No quedan temporales y se listan.
    assert_eq!(respaldos::listar(&carpeta).unwrap().len(), 1);
    assert_eq!(std::fs::read_dir(&carpeta).unwrap().count(), 1);
    // Dos respaldos en el mismo minuto no se pisan.
    let otro = respaldos::respaldar(&pool, &carpeta, &ZonaLocal, Utc::now(), None)
        .await
        .unwrap();
    assert_ne!(otro.ruta, r.ruta);
    assert!(!respaldos::toca_automatico(&carpeta, Utc::now()));
    assert!(respaldos::toca_automatico(
        &carpeta,
        Utc::now() + Duration::hours(25)
    ));
    assert!(respaldos::toca_automatico(
        &dir.path().join("vacia"),
        Utc::now()
    ));
}

#[tokio::test]
async fn conserva_los_ultimos_n() {
    let dir = tempfile::tempdir().unwrap();
    // Cada uno con su fecha de archivo; el quinto y su "-2" son del mismo minuto.
    let base = std::time::SystemTime::now() - std::time::Duration::from_secs(10 * 86_400);
    let nombres = [
        "chups-respaldo-2026-11-01-1200.db",
        "chups-respaldo-2026-11-02-1200.db",
        "chups-respaldo-2026-11-03-1200.db",
        "chups-respaldo-2026-11-04-1200.db",
        "chups-respaldo-2026-11-05-1200.db",
        "chups-respaldo-2026-11-05-1200-2.db",
    ];
    for (i, n) in nombres.iter().enumerate() {
        let ruta = dir.path().join(n);
        std::fs::write(&ruta, b"x").unwrap();
        let fecha = base + std::time::Duration::from_secs(86_400 * i.min(4) as u64);
        std::fs::File::options()
            .write(true)
            .open(&ruta)
            .unwrap()
            .set_modified(fecha)
            .unwrap();
    }
    std::fs::write(dir.path().join("otro-archivo.db"), b"x").unwrap();
    assert_eq!(respaldos::rotar(dir.path(), 3).unwrap(), 3);
    let quedan: Vec<String> = respaldos::listar(dir.path())
        .unwrap()
        .into_iter()
        .map(|r| r.nombre)
        .collect();
    assert_eq!(
        quedan,
        [
            "chups-respaldo-2026-11-05-1200-2.db",
            "chups-respaldo-2026-11-05-1200.db",
            "chups-respaldo-2026-11-04-1200.db"
        ]
    );
    // Los archivos que no son respaldos no se tocan.
    assert!(dir.path().join("otro-archivo.db").exists());
}

#[tokio::test]
async fn rechaza_archivos_ajenos_y_esquemas_mas_nuevos() {
    let dir = tempfile::tempdir().unwrap();
    let texto = dir.path().join("notas.db");
    std::fs::write(&texto, "esto no es una base").unwrap();
    assert!(matches!(
        respaldos::inspeccionar(&texto).await,
        Err(Error::Validacion(m)) if m.contains("no es una base de datos de CHUPS")
    ));

    // Una base SQLite de otro programa.
    let ajena = dir.path().join("ajena.db");
    let pool = SqlitePoolOptions::new()
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&ajena)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::query("CREATE TABLE clientes (id INTEGER PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    assert!(matches!(
        respaldos::inspeccionar(&ajena).await,
        Err(Error::Validacion(m)) if m.contains("no es una base de datos de CHUPS")
    ));

    // Un respaldo de una versión más nueva de la app.
    let pool = db::abrir(&dir.path().join("actual.db")).await.unwrap();
    let r = respaldos::respaldar(&pool, &dir.path().join("r"), &ZonaLocal, Utc::now(), None)
        .await
        .unwrap();
    pool.close().await;
    let copia = SqlitePoolOptions::new()
        .connect_with(SqliteConnectOptions::new().filename(&r.ruta))
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
         VALUES (99, 'del futuro', 1, x'00', 0)",
    )
    .execute(&copia)
    .await
    .unwrap();
    copia.close().await;
    assert!(matches!(
        respaldos::inspeccionar(Path::new(&r.ruta)).await,
        Err(Error::Validacion(m)) if m.contains("versión más nueva")
    ));
}

/// Un respaldo de la fase 5 (esquema 6) se restaura sobre la base actual y al abrirla migra.
#[tokio::test]
async fn restaura_un_respaldo_antiguo_que_luego_migra() {
    let dir = tempfile::tempdir().unwrap();
    // El respaldo antiguo: migraciones 0001 a 0006 copiadas tal cual (mismo checksum).
    let origen = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let hasta_6 = dir.path().join("migraciones");
    std::fs::create_dir(&hasta_6).unwrap();
    for e in std::fs::read_dir(&origen).unwrap() {
        let n = e.unwrap().file_name().into_string().unwrap();
        if n.as_str() <= "0006_inventario_y_fiados.sql" {
            std::fs::copy(origen.join(&n), hasta_6.join(&n)).unwrap();
        }
    }
    let antiguo = dir.path().join("chups-respaldo-2026-10-01-2000.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&antiguo)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    Migrator::new(hasta_6.as_path())
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE negocio SET nombre = 'Almacén de octubre';
         INSERT INTO productos (id, nombre, precio_venta, costo) VALUES (1, 'Leche', 1000, 700);
         INSERT INTO turnos (id, monto_inicial) VALUES (1, 0);
         INSERT INTO ventas (id, folio, subtotal, total, medio_pago, estado, turno_id)
             VALUES (1, 1, 1000, 1000, 'debito', 'pagada', 1);
         INSERT INTO venta_items (venta_id, producto_id, nombre, precio_unitario, cantidad_mil, subtotal)
             VALUES (1, 1, 'Leche', 1000, 1000, 1000);",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    let info = respaldos::inspeccionar(&antiguo).await.unwrap();
    assert_eq!(
        (info.version_esquema, info.version_app),
        (6, db::version_app())
    );
    assert_eq!((info.productos, info.ventas), (1, 1));
    assert_eq!(info.negocio.as_deref(), Some("Almacén de octubre"));
    assert!(info.ultima_venta.is_some());

    // La base actual, con otros datos y su -wal abierto hasta cerrar el pool.
    let actual = dir.path().join("chups-pos.db");
    let pool = db::abrir(&actual).await.unwrap();
    comun::crear(&pool, "X", "Otro producto", 500, 3).await;
    pool.close().await;
    respaldos::reemplazar_base(&actual, &antiguo).unwrap();

    let pool = db::abrir(&actual).await.unwrap();
    assert_eq!(db::version_esquema(&pool).await.unwrap(), db::version_app());
    let nombres: Vec<String> = sqlx::query_scalar("SELECT nombre FROM productos")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(nombres, ["Leche"]);
    // La migración 0007 rellenó el costo de la venta antigua como estimado.
    let costo: (Option<i64>, bool) =
        sqlx::query_as("SELECT costo_unitario, costo_estimado FROM venta_items")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(costo, (Some(700), true));
}

// --- Parte 6: cuándo toca una copia, la copia a un archivo y el formato .chups.

fn hace(horas: i64) -> Option<chrono::DateTime<Utc>> {
    Some(Utc::now() - Duration::hours(horas))
}

#[test]
fn la_frecuencia_decide_cuando_toca_una_copia() {
    use pos_core::servicios::config::FrecuenciaRespaldos::{
        AlCerrarCaja, CadaHora, SoloManual, UnaVezALaSemana, UnaVezAlDia,
    };
    use pos_core::servicios::respaldos::{
        toca,
        Momento::{Arranque, CierreDeCaja, Periodico},
    };
    let ahora = Utc::now();

    // Al cerrar la caja (lo de siempre): al cerrar el turno siempre; al abrir, si pasaron más de 24 horas.
    assert!(toca(AlCerrarCaja, CierreDeCaja, hace(1), ahora));
    assert!(toca(AlCerrarCaja, CierreDeCaja, None, ahora));
    assert!(!toca(AlCerrarCaja, Arranque, hace(23), ahora));
    assert!(toca(AlCerrarCaja, Arranque, hace(25), ahora));
    assert!(toca(AlCerrarCaja, Arranque, None, ahora));

    // Una vez al día: al abrir o al cerrar, solo si el último tiene más de 24 horas.
    assert!(!toca(UnaVezAlDia, CierreDeCaja, hace(5), ahora));
    assert!(!toca(UnaVezAlDia, Arranque, hace(23), ahora));
    assert!(toca(UnaVezAlDia, CierreDeCaja, hace(25), ahora));
    assert!(toca(UnaVezAlDia, Arranque, None, ahora));

    // Una vez a la semana.
    assert!(!toca(UnaVezALaSemana, Arranque, hace(24 * 6), ahora));
    assert!(!toca(UnaVezALaSemana, CierreDeCaja, hace(24 * 6), ahora));
    assert!(toca(UnaVezALaSemana, Arranque, hace(24 * 8), ahora));
    assert!(toca(UnaVezALaSemana, CierreDeCaja, None, ahora));

    // Solo manual: nunca, ni sin ninguna copia.
    for momento in [Arranque, CierreDeCaja, Periodico] {
        assert!(!toca(SoloManual, momento, None, ahora));
        assert!(!toca(SoloManual, momento, hace(24 * 365), ahora));
    }

    // Cada hora: en cualquier momento, si la última tiene más de una hora (o no hay ninguna).
    for momento in [Arranque, CierreDeCaja, Periodico] {
        assert!(!toca(
            CadaHora,
            momento,
            Some(ahora - Duration::minutes(59)),
            ahora
        ));
        assert!(toca(
            CadaHora,
            momento,
            Some(ahora - Duration::minutes(61)),
            ahora
        ));
        assert!(toca(CadaHora, momento, None, ahora));
    }

    // El temporizador no hace copias con las otras frecuencias, por viejas que sean las copias.
    for f in [AlCerrarCaja, UnaVezAlDia, UnaVezALaSemana] {
        assert!(!toca(f, Periodico, None, ahora));
        assert!(!toca(f, Periodico, hace(24 * 365), ahora));
    }
}

#[test]
fn la_rotacion_no_borra_las_copias_de_antes_de_restaurar() {
    let dir = tempfile::tempdir().unwrap();
    let base = std::time::SystemTime::now() - std::time::Duration::from_secs(86_400 * 10);
    // Una copia de antes de restaurar, la más vieja de todas, y siete copias por hora.
    let mut nombres = vec!["chups-respaldo-2026-11-01-0900-antes-de-restaurar.db".to_string()];
    nombres.extend((1..=7).map(|h| format!("chups-respaldo-2026-11-02-{h:02}00.db")));
    for (i, n) in nombres.iter().enumerate() {
        let ruta = dir.path().join(n);
        std::fs::write(&ruta, b"x").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&ruta)
            .unwrap()
            .set_modified(base + std::time::Duration::from_secs(3_600 * i as u64))
            .unwrap();
    }
    // Se conservan las 5 últimas y se borran 2; la de antes de restaurar queda, y no cuenta entre las 5.
    assert_eq!(respaldos::rotar(dir.path(), 5).unwrap(), 2);
    let quedan: Vec<String> = respaldos::listar(dir.path())
        .unwrap()
        .into_iter()
        .map(|r| r.nombre)
        .collect();
    assert_eq!(quedan.len(), 6);
    assert!(quedan.contains(&"chups-respaldo-2026-11-01-0900-antes-de-restaurar.db".to_string()));
    assert!(quedan.contains(&"chups-respaldo-2026-11-02-0700.db".to_string()));
    assert!(!quedan.contains(&"chups-respaldo-2026-11-02-0100.db".to_string()));
}

#[tokio::test]
async fn el_comportamiento_de_siempre_al_abrir_no_cambia() {
    let dir = tempfile::tempdir().unwrap();
    let carpeta = dir.path().join("Respaldos");
    // Sin copias: toca. Con una de hace un momento: no. Con una de hace dos días: sí.
    assert!(respaldos::toca_automatico(&carpeta, Utc::now()));
    let pool = db::abrir(&dir.path().join("chups-pos.db")).await.unwrap();
    respaldos::respaldar(&pool, &carpeta, &ZonaLocal, Utc::now(), None)
        .await
        .unwrap();
    assert!(!respaldos::toca_automatico(&carpeta, Utc::now()));
    assert!(respaldos::toca_automatico(
        &carpeta,
        Utc::now() + Duration::hours(49)
    ));
    assert!(respaldos::fecha_del_ultimo(&carpeta).is_some());
}

#[tokio::test]
async fn una_copia_chups_se_importa_igual_que_una_db() {
    let dir = tempfile::tempdir().unwrap();
    let pool = db::abrir(&dir.path().join("chups-pos.db")).await.unwrap();
    comun::crear(&pool, "A", "Leche", 1190, 10).await;
    comun::crear(&pool, "B", "Pan", 1500, 5).await;

    // La copia que se lleva a otro computador: el nombre propuesto lleva la extensión .chups.
    let nombre = respaldos::nombre_de_copia(&ZonaLocal, Utc::now());
    assert!(
        nombre.starts_with("chups-respaldo-") && nombre.ends_with(".chups"),
        "{nombre}"
    );
    let destino = dir.path().join("pendrive").join(&nombre);
    let r = respaldos::exportar_copia(&pool, &destino, Utc::now())
        .await
        .unwrap();
    assert!(r.bytes > 0 && std::path::Path::new(&r.ruta).exists());
    // Sin temporales a medias.
    let sobrantes: Vec<_> = std::fs::read_dir(destino.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(sobrantes, std::slice::from_ref(&nombre));

    // Lo mismo, con la extensión .db (el respaldo automático de siempre).
    let como_db = dir.path().join("pendrive").join("copia.db");
    std::fs::copy(&destino, &como_db).unwrap();
    let con_chups = respaldos::inspeccionar(&destino).await.unwrap();
    let con_db = respaldos::inspeccionar(&como_db).await.unwrap();
    assert_eq!((con_chups.productos, con_chups.ventas), (2, 0));
    assert_eq!(
        (
            con_chups.productos,
            con_chups.ventas,
            con_chups.version_esquema
        ),
        (con_db.productos, con_db.ventas, con_db.version_esquema)
    );

    // Y se restaura: la base de destino queda con los mismos datos.
    for origen in [&destino, &como_db] {
        let base = dir.path().join(format!(
            "restaurada-{}.db",
            origen.extension().unwrap().to_string_lossy()
        ));
        std::fs::write(&base, b"otra cosa").unwrap();
        respaldos::reemplazar_base(&base, origen).unwrap();
        let abierta = db::abrir(&base).await.unwrap();
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM productos")
            .fetch_one(&abierta)
            .await
            .unwrap();
        assert_eq!(total, 2);
        abierta.close().await;
    }

    // Una copia .chups dentro de la carpeta de respaldos también se lista (y se rota con las demás).
    let carpeta = dir.path().join("Respaldos");
    std::fs::create_dir_all(&carpeta).unwrap();
    std::fs::copy(&destino, carpeta.join(&nombre)).unwrap();
    assert_eq!(respaldos::listar(&carpeta).unwrap().len(), 1);
}

#[tokio::test]
async fn una_copia_a_un_archivo_reemplaza_la_que_ya_estaba() {
    let dir = tempfile::tempdir().unwrap();
    let pool = db::abrir(&dir.path().join("chups-pos.db")).await.unwrap();
    let destino = dir.path().join("mi-copia.chups");
    std::fs::write(&destino, b"vieja").unwrap();
    respaldos::exportar_copia(&pool, &destino, Utc::now())
        .await
        .unwrap();
    assert!(respaldos::inspeccionar(&destino).await.is_ok());
}
