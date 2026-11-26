//! Respaldos de la base: una copia consistente con `VACUUM INTO` (sirve aunque la app esté
//! vendiendo: SQLite copia una foto de la base), verificada con `PRAGMA integrity_check` antes de
//! darla por buena, con nombre `chups-respaldo-AAAA-MM-DD-HHMM.db`, y se conservan los últimos N.
//!
//! Restaurar: [`inspeccionar`] revisa que el archivo sea una base de CHUPS POS de esta versión o
//! anterior (las migraciones la suben al abrirla) y [`reemplazar_base`] la pone en lugar de la
//! actual una vez cerrado el pool. Quien llama respalda antes la base actual y reinicia la app.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use crate::error::validacion;
use crate::servicios::config::FrecuenciaRespaldos;
use crate::zona::Zona;
use crate::{db, Error, Resultado};

pub const PREFIJO: &str = "chups-respaldo-";
/// Extensión de las copias que se llevan a otro computador (sigue siendo SQLite). Las automáticas son `.db`.
pub const EXTENSION_COPIA: &str = "chups";
/// Una copia semanal se hace si la última tiene más de esto.
pub const CADA_HORAS_SEMANAL: i64 = 24 * 7;
/// Un respaldo automático se hace si el último tiene más de esto.
pub const CADA_HORAS: i64 = 24;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Respaldo {
    pub nombre: String,
    pub ruta: String,
    pub bytes: u64,
    /// Cuándo se escribió (UTC).
    pub fecha: String,
}

fn io(que: &str, e: std::io::Error) -> Error {
    Error::Validacion(format!("{que}: {e}"))
}

fn a_utc(t: SystemTime) -> DateTime<Utc> {
    DateTime::<Utc>::from(t)
}

/// Los respaldos de la carpeta, del más nuevo al más antiguo: por la fecha del archivo y, si es
/// la misma, por el nombre sin ".db" (así "…-1942-3" va antes que "…-1942").
pub fn listar(carpeta: &Path) -> Resultado<Vec<Respaldo>> {
    let Ok(entradas) = std::fs::read_dir(carpeta) else {
        return Ok(Vec::new());
    };
    let mut v: Vec<Respaldo> = entradas
        .filter_map(Result::ok)
        .filter_map(|e| {
            let nombre = e.file_name().to_string_lossy().into_owned();
            if !nombre.starts_with(PREFIJO)
                || !(nombre.ends_with(".db") || nombre.ends_with(".chups"))
            {
                return None;
            }
            let meta = e.metadata().ok()?;
            Some(Respaldo {
                ruta: e.path().to_string_lossy().into_owned(),
                bytes: meta.len(),
                fecha: crate::zona::a_texto(a_utc(meta.modified().ok()?)),
                nombre,
            })
        })
        .collect();
    let tronco = |r: &Respaldo| {
        r.nombre
            .trim_end_matches(".db")
            .trim_end_matches(".chups")
            .to_owned()
    };
    v.sort_by(|a, b| {
        b.fecha
            .cmp(&a.fecha)
            .then_with(|| tronco(b).cmp(&tronco(a)))
    });
    Ok(v)
}

/// Borra los respaldos más antiguos y deja los `conservar` más nuevos. Devuelve cuántos borró.
pub fn rotar(carpeta: &Path, conservar: usize) -> Resultado<usize> {
    let mut borrados = 0;
    for r in listar(carpeta)?.into_iter().skip(conservar.max(1)) {
        std::fs::remove_file(&r.ruta)
            .map_err(|e| io("No se pudo borrar un respaldo antiguo", e))?;
        borrados += 1;
    }
    Ok(borrados)
}

/// Cuándo se evalúa si toca un respaldo automático.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Momento {
    /// La app acaba de abrirse.
    Arranque,
    /// Se acaba de cerrar un turno de caja.
    CierreDeCaja,
}

/// Cuándo se hizo el último respaldo de la carpeta (UTC); `None` si no hay ninguno.
pub fn fecha_del_ultimo(carpeta: &Path) -> Option<DateTime<Utc>> {
    let r = listar(carpeta).ok()?.into_iter().next()?;
    crate::zona::de_texto(&r.fecha)
}

/**
 * ¿Toca un respaldo automático? Depende de lo que se eligió en Configuración:
 * - al cerrar la caja (lo de siempre): al cerrar el turno, siempre; al abrir la app, si no hay ninguno o el
 *   último tiene más de 24 horas;
 * - una vez al día: al abrir la app o al cerrar el turno, si el último tiene más de 24 horas;
 * - una vez a la semana: lo mismo con 7 días;
 * - solo manual: nunca.
 */
pub fn toca(
    frecuencia: FrecuenciaRespaldos,
    momento: Momento,
    ultimo: Option<DateTime<Utc>>,
    ahora: DateTime<Utc>,
) -> bool {
    let vencido = |horas: i64| ultimo.is_none_or(|f| ahora - f > Duration::hours(horas));
    match (frecuencia, momento) {
        (FrecuenciaRespaldos::SoloManual, _) => false,
        (FrecuenciaRespaldos::AlCerrarCaja, Momento::CierreDeCaja) => true,
        (FrecuenciaRespaldos::AlCerrarCaja, Momento::Arranque)
        | (FrecuenciaRespaldos::UnaVezAlDia, _) => vencido(CADA_HORAS),
        (FrecuenciaRespaldos::UnaVezALaSemana, _) => vencido(CADA_HORAS_SEMANAL),
    }
}

/// ¿Toca al abrir la app con la frecuencia de siempre? Sí si no hay ninguno o el último tiene más de 24 horas.
pub fn toca_automatico(carpeta: &Path, ahora: DateTime<Utc>) -> bool {
    toca(
        FrecuenciaRespaldos::AlCerrarCaja,
        Momento::Arranque,
        fecha_del_ultimo(carpeta),
        ahora,
    )
}

/// Abre un archivo de base solo para leer, sin migrar ni crear nada.
async fn abrir_solo_lectura(ruta: &Path) -> Resultado<SqlitePool> {
    let opciones = SqliteConnectOptions::new()
        .filename(ruta)
        .read_only(true)
        .create_if_missing(false);
    Ok(SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opciones)
        .await?)
}

async fn integridad(pool: &SqlitePool) -> Resultado<bool> {
    let r: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(pool)
        .await?;
    Ok(r == "ok")
}

/// Copia la base con `VACUUM INTO` a `destino`: primero a un temporal, la verifica y recién entonces le pone su
/// nombre (un archivo que ya estaba se reemplaza). Devuelve los bytes de la copia.
async fn copiar_verificada(pool: &SqlitePool, destino: &Path) -> Resultado<u64> {
    let temporal = destino.with_extension(format!(
        "{}.tmp",
        destino.extension().and_then(|e| e.to_str()).unwrap_or("db")
    ));
    let _ = std::fs::remove_file(&temporal);
    sqlx::query("VACUUM INTO ?")
        .bind(temporal.to_string_lossy().as_ref())
        .execute(pool)
        .await
        .map_err(|e| Error::Validacion(format!("No se pudo copiar la base: {e}")))?;
    let copia = abrir_solo_lectura(&temporal).await?;
    let sana = integridad(&copia).await;
    copia.close().await;
    if !matches!(sana, Ok(true)) {
        let _ = std::fs::remove_file(&temporal);
        return validacion("La copia del respaldo no pasó la verificación de integridad");
    }
    std::fs::rename(&temporal, destino).map_err(|e| io("No se pudo guardar el respaldo", e))?;
    Ok(std::fs::metadata(destino)
        .map_err(|e| io("No se pudo leer el respaldo", e))?
        .len())
}

/// El nombre que se propone para una copia que se lleva a otro computador:
/// `chups-respaldo-AAAA-MM-DD-HHMM.chups`.
pub fn nombre_de_copia(zona: &dyn Zona, ahora: DateTime<Utc>) -> String {
    format!(
        "{PREFIJO}{}.{EXTENSION_COPIA}",
        zona.a_local(ahora).format("%Y-%m-%d-%H%M")
    )
}

/// Guarda una copia completa de la base en el archivo que se eligió (un pendrive, por ejemplo). Es la misma
/// copia verificada de siempre: se puede importar en otro computador con "Importar una copia".
pub async fn exportar_copia(
    pool: &SqlitePool,
    destino: &Path,
    ahora: DateTime<Utc>,
) -> Resultado<Respaldo> {
    if let Some(carpeta) = destino.parent().filter(|c| !c.as_os_str().is_empty()) {
        std::fs::create_dir_all(carpeta).map_err(|e| {
            io(
                &format!("No se pudo crear la carpeta {}", carpeta.display()),
                e,
            )
        })?;
    }
    let bytes = copiar_verificada(pool, destino).await?;
    Ok(Respaldo {
        nombre: destino
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        bytes,
        fecha: crate::zona::a_texto(ahora),
        ruta: destino.to_string_lossy().into_owned(),
    })
}

/// Respalda la base en `carpeta`: copia con `VACUUM INTO` a un archivo temporal, la verifica y
/// recién entonces le pone su nombre. `sufijo` se agrega al nombre ("antes-de-restaurar").
pub async fn respaldar(
    pool: &SqlitePool,
    carpeta: &Path,
    zona: &dyn Zona,
    ahora: DateTime<Utc>,
    sufijo: Option<&str>,
) -> Resultado<Respaldo> {
    std::fs::create_dir_all(carpeta).map_err(|e| {
        io(
            &format!("No se pudo crear la carpeta {}", carpeta.display()),
            e,
        )
    })?;
    let base = format!(
        "{PREFIJO}{}{}",
        zona.a_local(ahora).format("%Y-%m-%d-%H%M"),
        sufijo.map(|s| format!("-{s}")).unwrap_or_default()
    );
    let mut ruta = carpeta.join(format!("{base}.db"));
    let mut n = 2;
    while ruta.exists() {
        ruta = carpeta.join(format!("{base}-{n}.db"));
        n += 1;
    }
    let bytes = copiar_verificada(pool, &ruta).await?;
    let meta = std::fs::metadata(&ruta).map_err(|e| io("No se pudo leer el respaldo", e))?;
    Ok(Respaldo {
        nombre: ruta
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        bytes,
        fecha: crate::zona::a_texto(meta.modified().map(a_utc).unwrap_or(ahora)),
        ruta: ruta.to_string_lossy().into_owned(),
    })
}

/// Lo que se muestra antes de restaurar un archivo.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InfoRespaldo {
    pub ruta: String,
    pub bytes: u64,
    /// Cuándo se escribió el archivo (UTC).
    pub fecha: String,
    pub version_esquema: i64,
    /// La versión de esta app: un respaldo más antiguo se pone al día al abrirlo.
    pub version_app: i64,
    pub negocio: Option<String>,
    pub productos: i64,
    pub ventas: i64,
    /// Fecha (UTC) de la última venta.
    pub ultima_venta: Option<String>,
}

/// Revisa que `ruta` sea una base de CHUPS POS que esta versión puede abrir: SQLite sana, con
/// las tablas de la app, migraciones conocidas (mismos números y contenido) y un esquema que no
/// sea más nuevo que el de la app.
pub async fn inspeccionar(ruta: &Path) -> Resultado<InfoRespaldo> {
    let ajeno = || Error::Validacion("El archivo no es una base de datos de CHUPS POS".into());
    let meta = std::fs::metadata(ruta).map_err(|e| io("No se pudo leer el archivo", e))?;
    let mut cabecera = [0u8; 16];
    {
        use std::io::Read;
        let mut f = std::fs::File::open(ruta).map_err(|e| io("No se pudo abrir el archivo", e))?;
        f.read_exact(&mut cabecera).map_err(|_| ajeno())?;
    }
    if &cabecera != b"SQLite format 3\0" {
        return Err(ajeno());
    }
    let pool = abrir_solo_lectura(ruta).await.map_err(|_| ajeno())?;
    let resultado = async {
        let tablas: Vec<String> =
            sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table'")
                .fetch_all(&pool)
                .await
                .map_err(|_| ajeno())?;
        for t in ["_sqlx_migrations", "productos", "ventas", "negocio", "config"] {
            if !tablas.iter().any(|n| n == t) {
                return Err(ajeno());
            }
        }
        let aplicadas: Vec<(i64, Vec<u8>)> = sqlx::query_as(
            "SELECT version, checksum FROM _sqlx_migrations WHERE success = 1 ORDER BY version",
        )
        .fetch_all(&pool)
        .await?;
        let version = aplicadas.last().map_or(0, |a| a.0);
        if version > db::version_app() {
            return validacion(format!(
                "El respaldo es de una versión más nueva de la app (esquema {version}; esta versión llega al {}). Actualiza CHUPS POS antes de restaurarlo",
                db::version_app()
            ));
        }
        for (v, checksum) in &aplicadas {
            let conocida = db::MIGRACIONES.iter().find(|m| m.version == *v);
            if conocida.is_none_or(|m| m.checksum.as_ref() != checksum.as_slice()) {
                return validacion(
                    "El respaldo tiene cambios en la base que esta versión no reconoce: no se puede restaurar",
                );
            }
        }
        if !integridad(&pool).await? {
            return validacion("El respaldo está dañado: no pasó la verificación de integridad");
        }
        let negocio: Option<String> = sqlx::query_scalar("SELECT nombre FROM negocio WHERE id = 1")
            .fetch_optional(&pool)
            .await?
            .flatten();
        let (productos, ventas, ultima): (i64, i64, Option<String>) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM productos), (SELECT COUNT(*) FROM ventas),
                    (SELECT MAX(fecha_hora) FROM ventas)",
        )
        .fetch_one(&pool)
        .await?;
        Ok(InfoRespaldo {
            ruta: ruta.to_string_lossy().into_owned(),
            bytes: meta.len(),
            fecha: crate::zona::a_texto(meta.modified().map(a_utc).unwrap_or_else(|_| Utc::now())),
            version_esquema: version,
            version_app: db::version_app(),
            negocio: negocio.filter(|n| !n.trim().is_empty()),
            productos,
            ventas,
            ultima_venta: ultima,
        })
    }
    .await;
    pool.close().await;
    resultado
}

/// Pone `nueva` en lugar de la base `destino` (con el pool ya cerrado): copia primero a un
/// temporal junto al destino y lo renombra encima, y borra los -wal y -shm de la base anterior
/// (si quedaran, SQLite los aplicaría sobre la restaurada). Es trabajo de disco: fuera de los
/// hilos async.
pub fn reemplazar_base(destino: &Path, nueva: &Path) -> Resultado<()> {
    let temporal: PathBuf = destino.with_extension("db.restaurando");
    std::fs::copy(nueva, &temporal).map_err(|e| io("No se pudo copiar el respaldo", e))?;
    for sufijo in ["-wal", "-shm"] {
        let mut otro = destino.as_os_str().to_owned();
        otro.push(sufijo);
        let otro = PathBuf::from(otro);
        // Al cerrar el pool SQLite los borra solo, pero una conexión puede tardar un momento en
        // soltarlos (en Windows el archivo sigue "en uso" mientras tanto): se reintenta hasta 5 s.
        let mut intento = 0;
        while otro.exists() {
            match std::fs::remove_file(&otro) {
                Ok(()) => break,
                Err(e) if intento >= 50 => {
                    let _ = std::fs::remove_file(&temporal);
                    return Err(io("No se pudo cerrar la base actual (¿sigue abierta?)", e));
                }
                Err(_) => {
                    intento += 1;
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
    }
    std::fs::rename(&temporal, destino).map_err(|e| io("No se pudo reemplazar la base", e))?;
    Ok(())
}

/// Tamaño de la base en uso (con su -wal).
pub fn tamano_base(ruta: &Path) -> u64 {
    let mut wal = ruta.as_os_str().to_owned();
    wal.push("-wal");
    [ruta.to_path_buf(), PathBuf::from(wal)]
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum()
}
