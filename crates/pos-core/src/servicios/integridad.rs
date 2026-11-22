//! Revisión de la base después de un cierre brusco (un corte de luz, por ejemplo).
//!
//! Al abrir la app, si la vez anterior no se cerró bien, se corre `PRAGMA quick_check`; si falla,
//! se ofrece restaurar el último respaldo sano. SQLite con WAL casi nunca queda dañado por un
//! corte, pero un disco con problemas sí puede dañarlo: por eso se revisa solo cuando hace falta
//! (en una base grande la revisión tarda) y no en cada arranque.

use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

use crate::servicios::respaldos::{self, Respaldo};
use crate::{Error, Resultado};

/// Cuántos mensajes del problema se guardan (`quick_check` puede devolver cientos).
const MAX_MENSAJES: usize = 5;

/// `PRAGMA quick_check`: los problemas encontrados (vacío si la base está sana). Si la base está
/// tan dañada que ni se puede consultar, el error de SQLite es el problema.
pub async fn revisar(pool: &SqlitePool) -> Vec<String> {
    let filas: Result<Vec<String>, sqlx::Error> =
        sqlx::query_scalar(&format!("PRAGMA quick_check({MAX_MENSAJES})"))
            .fetch_all(pool)
            .await;
    match filas {
        Ok(filas) if filas.len() == 1 && filas[0] == "ok" => Vec::new(),
        Ok(filas) => filas.into_iter().take(MAX_MENSAJES).collect(),
        Err(e) => vec![e.to_string()],
    }
}

/// El respaldo más nuevo que pasa la verificación completa (esquema conocido e integridad); los
/// dañados se saltan.
pub async fn ultimo_sano(carpeta: &Path) -> Resultado<Option<Respaldo>> {
    for respaldo in respaldos::listar(carpeta)? {
        if respaldos::inspeccionar(Path::new(&respaldo.ruta))
            .await
            .is_ok()
        {
            return Ok(Some(respaldo));
        }
    }
    Ok(None)
}

/// Copia la base dañada (tal cual, con su -wal) a `carpeta`, para no perderla al restaurar: quizá
/// alguien pueda rescatar algo de ahí. Devuelve la ruta de la copia.
pub fn guardar_danada(base: &Path, carpeta: &Path, sello: &str) -> Resultado<PathBuf> {
    let io =
        |e: std::io::Error| Error::Validacion(format!("No se pudo guardar la base dañada: {e}"));
    std::fs::create_dir_all(carpeta).map_err(io)?;
    let destino = carpeta.join(format!("chups-pos-danada-{sello}.db"));
    std::fs::copy(base, &destino).map_err(io)?;
    let mut wal = base.as_os_str().to_owned();
    wal.push("-wal");
    if Path::new(&wal).exists() {
        let mut copia = destino.as_os_str().to_owned();
        copia.push("-wal");
        let _ = std::fs::copy(&wal, copia);
    }
    Ok(destino)
}
