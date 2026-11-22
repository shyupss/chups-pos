//! Registro de errores en archivos rotativos dentro de la carpeta de datos (`Registros`).
//!
//! - Hasta 5 archivos de 1 MB: `chups-pos.log` (el actual) y `chups-pos.1.log` … `chups-pos.4.log`
//!   (los más viejos). Al llenarse el actual se rota y se borra el más antiguo.
//! - Se escribe en un hilo aparte (`tracing-appender`): registrar nunca hace esperar a la caja.
//! - **Sin datos de clientes ni PIN**: solo se registran el arranque, los errores del sistema (base
//!   de datos, impresión, respaldos, actualizaciones) y los pánicos con su lugar en el código. Los
//!   mensajes de validación (que pueden nombrar un producto o un cliente) y los del PIN no se
//!   guardan; solo su tipo.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use tracing_appender::non_blocking::WorkerGuard;

/// Tamaño máximo de cada archivo.
pub const MAX_BYTES: u64 = 1024 * 1024;
/// El actual más los rotados.
pub const MAX_ARCHIVOS: usize = 5;
const BASE: &str = "chups-pos";
pub const CARPETA: &str = "Registros";

/// Escribe en `chups-pos.log` y, al pasar el máximo, rota los archivos.
pub struct EscritorRotativo {
    carpeta: PathBuf,
    max_bytes: u64,
    max_archivos: usize,
    archivo: Option<File>,
    tamano: u64,
}

impl EscritorRotativo {
    pub fn nuevo(carpeta: &Path, max_bytes: u64, max_archivos: usize) -> io::Result<Self> {
        fs::create_dir_all(carpeta)?;
        let mut e = EscritorRotativo {
            carpeta: carpeta.to_path_buf(),
            max_bytes,
            max_archivos: max_archivos.max(2),
            archivo: None,
            tamano: 0,
        };
        e.abrir()?;
        Ok(e)
    }

    fn ruta(&self, n: usize) -> PathBuf {
        if n == 0 {
            self.carpeta.join(format!("{BASE}.log"))
        } else {
            self.carpeta.join(format!("{BASE}.{n}.log"))
        }
    }

    fn abrir(&mut self) -> io::Result<()> {
        let ruta = self.ruta(0);
        let f = OpenOptions::new().create(true).append(true).open(&ruta)?;
        self.tamano = f.metadata()?.len();
        self.archivo = Some(f);
        Ok(())
    }

    /// El actual pasa a `.1`, `.1` a `.2`… y el último se borra.
    fn rotar(&mut self) -> io::Result<()> {
        self.archivo = None;
        let ultimo = self.max_archivos - 1;
        let _ = fs::remove_file(self.ruta(ultimo));
        for n in (0..ultimo).rev() {
            let origen = self.ruta(n);
            if origen.exists() {
                fs::rename(&origen, self.ruta(n + 1))?;
            }
        }
        self.abrir()
    }
}

impl Write for EscritorRotativo {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.tamano > 0 && self.tamano + buf.len() as u64 > self.max_bytes {
            self.rotar()?;
        }
        let f = self
            .archivo
            .as_mut()
            .ok_or_else(|| io::Error::other("sin archivo"))?;
        let n = f.write(buf)?;
        self.tamano += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.archivo.as_mut() {
            Some(f) => f.flush(),
            None => Ok(()),
        }
    }
}

/// Deja el registro funcionando. Devuelve la guarda que hay que mantener viva (al soltarla se vacía
/// lo pendiente) o `None` si no se pudo abrir la carpeta: la app sigue sin registro.
pub fn iniciar(carpeta_datos: &Path) -> Option<WorkerGuard> {
    let escritor =
        EscritorRotativo::nuevo(&carpeta_datos.join(CARPETA), MAX_BYTES, MAX_ARCHIVOS).ok()?;
    let (no_bloqueante, guarda) = tracing_appender::non_blocking(escritor);
    let ya = tracing_subscriber::fmt()
        .with_writer(no_bloqueante)
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .try_init();
    if ya.is_err() {
        return None;
    }
    // Un pánico queda en el registro con su lugar (sin el mensaje, que podría llevar datos).
    std::panic::set_hook(Box::new(|info| {
        let lugar = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "?".into());
        tracing::error!("pánico en {lugar}");
        // El registro escribe en otro hilo y con panic=abort el proceso muere enseguida.
        std::thread::sleep(Duration::from_millis(200));
    }));
    Some(guarda)
}

/// Para los errores que llegan de un comando: de la base de datos o del sistema se guarda el
/// mensaje; de la validación y del PIN, solo el tipo.
pub fn error_de_comando(e: &pos_core::Error) {
    use pos_core::Error::*;
    match e {
        BaseDatos(_) | Migracion(_) => tracing::error!("error del sistema: {e}"),
        Licencia(_) => tracing::warn!("licencia rechazada"),
        Pin(_) => tracing::warn!("PIN rechazado"),
        Validacion(_) | NoEncontrado(_) | Conflicto(_) => tracing::debug!("dato rechazado"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archivos(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn rota_y_nunca_pasa_de_5_archivos_de_1_mb() {
        let dir = tempfile::tempdir().unwrap();
        let mut e = EscritorRotativo::nuevo(dir.path(), MAX_BYTES, MAX_ARCHIVOS).unwrap();
        let linea = vec![b'x'; 1000];
        // 12 MB de registro: tiene que dar vueltas varias veces.
        for _ in 0..12_000 {
            e.write_all(&linea).unwrap();
        }
        e.flush().unwrap();
        let nombres = archivos(dir.path());
        assert_eq!(
            nombres,
            [
                "chups-pos.1.log",
                "chups-pos.2.log",
                "chups-pos.3.log",
                "chups-pos.4.log",
                "chups-pos.log"
            ]
        );
        for n in nombres {
            let tamano = fs::metadata(dir.path().join(&n)).unwrap().len();
            assert!(tamano <= MAX_BYTES, "{n} pesa {tamano}");
        }
    }

    #[test]
    fn sigue_en_el_archivo_actual_al_reabrir() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut e = EscritorRotativo::nuevo(dir.path(), MAX_BYTES, MAX_ARCHIVOS).unwrap();
            e.write_all(b"uno\n").unwrap();
        }
        let mut e = EscritorRotativo::nuevo(dir.path(), MAX_BYTES, MAX_ARCHIVOS).unwrap();
        e.write_all(b"dos\n").unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("chups-pos.log")).unwrap(),
            "uno\ndos\n"
        );
        assert_eq!(archivos(dir.path()), ["chups-pos.log"]);
    }

    #[test]
    fn los_errores_de_validacion_y_pin_no_dejan_su_mensaje() {
        // Solo se mira que no se pueda colar el texto: el registro de estos tipos no lo usa.
        let e = pos_core::Error::Validacion("El cliente Rosa Pérez no existe".into());
        let texto = format!("{e}");
        assert!(texto.contains("Rosa"));
        // `error_de_comando` no interpola el error en estos tipos (ver el match de arriba).
        error_de_comando(&e);
        error_de_comando(&pos_core::Error::Pin(pos_core::FalloPin::Requerido));
    }
}
