// Robustez: la revisión de la base tras un cierre brusco, el último respaldo sano y la información
// para soporte (sin licencia, sin PIN, sin datos de clientes).

mod comun;

use std::path::Path;
use std::time::Duration;

use chrono::Utc;
use pos_core::db;
use pos_core::servicios::integridad;
use pos_core::servicios::licencia::{self, Contexto};
use pos_core::servicios::respaldos;
use pos_core::servicios::soporte::{self, DatosSoporte};
use pos_core::zona::ZonaLocal;
use pos_licencia::{clave_publica_de, firmar, generar_clave_privada, Huella, Licencia};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

/// Una base con bastantes páginas (para poder dañar el medio sin tocar la cabecera).
async fn base_con_datos(ruta: &Path) -> SqlitePool {
    let pool = db::abrir(ruta).await.unwrap();
    for i in 0..400 {
        comun::crear(
            &pool,
            &format!("78{i:05}"),
            &format!("Producto número {i}"),
            1000 + i,
            5,
        )
        .await;
    }
    pool
}

/// Abre sin migrar, como lo haría una revisión sobre una base que quizá esté dañada.
async fn abrir_crudo(ruta: &Path) -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(ruta)
                .create_if_missing(false),
        )
        .await
        .unwrap()
}

/// Cerrar el pool no es instantáneo: SQLite termina de pasar el WAL a la base en otro hilo. Hasta que
/// el -wal desaparece, el archivo de la base todavía puede cambiar.
async fn esperar_wal(ruta: &Path) {
    let mut wal = ruta.as_os_str().to_owned();
    wal.push("-wal");
    for _ in 0..100 {
        if !Path::new(&wal).exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Pisa con basura un tramo del medio de la base, como un sector malo del disco.
fn danar(ruta: &Path) {
    let mut bytes = std::fs::read(ruta).unwrap();
    assert!(bytes.len() > 4096 * 12, "la base de prueba es muy chica");
    let desde = 4096 * 6;
    for (i, b) in bytes[desde..desde + 4096 * 4].iter_mut().enumerate() {
        *b = (i * 31 % 251) as u8;
    }
    std::fs::write(ruta, bytes).unwrap();
}

#[tokio::test]
async fn una_base_sana_pasa_la_revision() {
    let dir = tempfile::tempdir().unwrap();
    let pool = base_con_datos(&dir.path().join("chups-pos.db")).await;
    assert!(integridad::revisar(&pool).await.is_empty());
}

#[tokio::test]
async fn una_base_danada_no_pasa_y_dice_por_que() {
    let dir = tempfile::tempdir().unwrap();
    let ruta = dir.path().join("chups-pos.db");
    let pool = base_con_datos(&ruta).await;
    pool.close().await;
    esperar_wal(&ruta).await;
    danar(&ruta);
    let problemas = integridad::revisar(&abrir_crudo(&ruta).await).await;
    assert!(
        !problemas.is_empty(),
        "una base con basura en el medio tiene que fallar"
    );
    assert!(problemas.len() <= 5);
}

#[tokio::test]
async fn el_ultimo_sano_se_salta_los_respaldos_danados() {
    let dir = tempfile::tempdir().unwrap();
    let carpeta = dir.path().join("Respaldos");
    let pool = base_con_datos(&dir.path().join("chups-pos.db")).await;

    // Uno sano (el más viejo) y otro, más nuevo, dañado.
    let sano = respaldos::respaldar(&pool, &carpeta, &ZonaLocal, Utc::now(), None)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let reciente = respaldos::respaldar(&pool, &carpeta, &ZonaLocal, Utc::now(), Some("b"))
        .await
        .unwrap();
    danar(Path::new(&reciente.ruta));

    let elegido = integridad::ultimo_sano(&carpeta).await.unwrap().unwrap();
    assert_eq!(
        elegido.nombre, sano.nombre,
        "tiene que saltarse el dañado, aunque sea más nuevo"
    );

    // Si todos están dañados, no hay ninguno.
    danar(Path::new(&sano.ruta));
    assert!(integridad::ultimo_sano(&carpeta).await.unwrap().is_none());
    // Y una carpeta que no existe tampoco da error.
    assert!(integridad::ultimo_sano(&dir.path().join("nada"))
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn la_base_danada_se_guarda_aparte_antes_de_restaurar() {
    let dir = tempfile::tempdir().unwrap();
    let ruta = dir.path().join("chups-pos.db");
    base_con_datos(&ruta).await.close().await;
    esperar_wal(&ruta).await;
    danar(&ruta);
    let copia = integridad::guardar_danada(&ruta, &dir.path().join("Respaldos"), "2027-01-02-0304")
        .unwrap();
    assert!(copia.exists());
    assert!(copia
        .file_name()
        .unwrap()
        .to_string_lossy()
        .contains("danada"));
    assert_eq!(
        std::fs::read(&copia).unwrap(),
        std::fs::read(&ruta).unwrap()
    );
}

#[tokio::test]
async fn restaurar_el_ultimo_sano_devuelve_una_base_que_abre_y_conserva_los_datos() {
    let dir = tempfile::tempdir().unwrap();
    let ruta = dir.path().join("chups-pos.db");
    let carpeta = dir.path().join("Respaldos");
    let pool = base_con_datos(&ruta).await;
    respaldos::respaldar(&pool, &carpeta, &ZonaLocal, Utc::now(), None)
        .await
        .unwrap();
    pool.close().await;
    esperar_wal(&ruta).await;
    danar(&ruta);
    assert!(!integridad::revisar(&abrir_crudo(&ruta).await)
        .await
        .is_empty());

    let sano = integridad::ultimo_sano(&carpeta).await.unwrap().unwrap();
    integridad::guardar_danada(&ruta, &carpeta, "x").unwrap();
    respaldos::reemplazar_base(&ruta, Path::new(&sano.ruta)).unwrap();

    let pool = db::abrir(&ruta).await.unwrap();
    assert!(integridad::revisar(&pool).await.is_empty());
    let productos: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM productos")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(productos, 400);
}

#[tokio::test]
async fn la_informacion_de_soporte_no_lleva_licencia_pin_ni_clientes() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let privada = generar_clave_privada().unwrap();
    let huella = Huella::de("id-de-prueba", "caja-1");
    let ctx = Contexto {
        clave_publica: clave_publica_de(&privada),
        huella,
        archivo_prueba: None,
        version: "1.0.0".into(),
        contacto: String::new(),
    };
    let texto_licencia = firmar(
        &Licencia::nueva("Almacén Secreto", Some("11.111.111-1"), 1, None).unwrap(),
        &privada,
    )
    .unwrap();
    let estado = licencia::activar(&pool, &ctx, &texto_licencia, 1_800_000_000)
        .await
        .unwrap();
    let datos = DatosSoporte {
        version_app: "1.0.0".into(),
        windows: "Windows 10 Pro 22H2 (compilación 19045)".into(),
        version_esquema: 7,
        impresora: Some("XP-80C".into()),
        ruta_datos: r"C:\Users\caja\AppData\Roaming\cl.chups.pos".into(),
        tamano_base_bytes: 3 * 1_048_576,
        licencia: estado,
    };
    let t = soporte::texto(&datos);
    for esperado in [
        "CHUPS POS: 1.0.0",
        "Windows 10 Pro",
        "v7",
        "activa",
        "XP-80C",
        "3.0 MB",
        "cl.chups.pos",
    ] {
        assert!(t.contains(esperado), "falta «{esperado}» en:\n{t}");
    }
    // Ni el texto de la licencia ni su firma, ni el nombre o el RUT del negocio.
    let sin_guiones = texto_licencia.replace('-', "");
    assert!(!t.contains(&texto_licencia) && !t.contains(&sin_guiones[5..30]));
    assert!(!t.contains("Almacén Secreto") && !t.contains("11.111.111-1"));
}
