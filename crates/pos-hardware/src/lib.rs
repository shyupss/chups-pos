//! Hardware del POS: impresoras instaladas en el sistema.
//!
//! - Windows: lista con `EnumPrintersW`, lee el estado de `PRINTER_INFO_2` y manda los bytes
//!   en modo RAW por la cola de impresión (el driver de la impresora no los toca).
//! - Linux (desarrollo): `lpstat` para listar y `lp -o raw` para imprimir.
//! - En todos: la impresora virtual "Archivo (pruebas)" escribe los bytes en un `.bin`, para
//!   probar sin impresora.
//!
//! Todas las funciones bloquean: quien las llama debe hacerlo fuera del hilo de la interfaz.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
use unix as sistema;
#[cfg(windows)]
use windows as sistema;

/// Nombre de la impresora virtual que escribe en archivos.
pub const IMPRESORA_ARCHIVO: &str = "Archivo (pruebas)";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("No se encontró la impresora «{0}». Revisa que esté encendida y conectada, y prueba de nuevo.")]
    NoEncontrada(String),
    #[error("{0}")]
    Sistema(String),
    #[error("No se pudo escribir el archivo de prueba: {0}")]
    Archivo(#[from] std::io::Error),
}

pub type Resultado<T> = Result<T, Error>;

/// Estado que informa el driver. Muchas térmicas USB no avisan cuando están apagadas:
/// en ese caso se ven como "lista" hasta que falla una impresión.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "tipo", content = "detalle", rename_all = "camelCase")]
pub enum EstadoImpresora {
    Lista,
    Desconectada,
    ConError(String),
    /// El sistema no informa el estado (por ejemplo, en Linux sin CUPS).
    Desconocido,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Impresora {
    pub nombre: String,
    pub predeterminada: bool,
    pub estado: EstadoImpresora,
    /// La impresora virtual de pruebas.
    pub es_virtual: bool,
}

/// Punto de entrada. Guarda dónde escribe la impresora virtual.
#[derive(Debug, Clone)]
pub struct Hardware {
    carpeta_pruebas: PathBuf,
}

impl Hardware {
    /// `carpeta_datos`: carpeta de datos de la app; la impresora virtual escribe en
    /// `carpeta_datos/impresora-pruebas`.
    pub fn new(carpeta_datos: impl AsRef<Path>) -> Self {
        Hardware {
            carpeta_pruebas: carpeta_datos.as_ref().join("impresora-pruebas"),
        }
    }

    pub fn carpeta_pruebas(&self) -> &Path {
        &self.carpeta_pruebas
    }

    /// Impresoras del sistema y, al final, la virtual de pruebas.
    pub fn listar(&self) -> Resultado<Vec<Impresora>> {
        let mut lista = sistema::listar()?;
        lista.push(Impresora {
            nombre: IMPRESORA_ARCHIVO.into(),
            predeterminada: false,
            estado: EstadoImpresora::Lista,
            es_virtual: true,
        });
        Ok(lista)
    }

    pub fn estado(&self, impresora: &str) -> Resultado<EstadoImpresora> {
        if impresora == IMPRESORA_ARCHIVO {
            return Ok(EstadoImpresora::Lista);
        }
        sistema::estado(impresora)
    }

    /// Manda los bytes tal cual (RAW). `documento` es el nombre que se ve en la cola de
    /// impresión y, en la impresora virtual, el nombre del archivo.
    pub fn enviar(&self, impresora: &str, documento: &str, bytes: &[u8]) -> Resultado<()> {
        if impresora == IMPRESORA_ARCHIVO {
            self.escribir_archivo(documento, bytes)?;
            return Ok(());
        }
        sistema::enviar(impresora, documento, bytes)
    }

    /// Escribe `<carpeta>/impresora-pruebas/<documento>.bin` y devuelve la ruta.
    pub fn escribir_archivo(&self, documento: &str, bytes: &[u8]) -> Resultado<PathBuf> {
        std::fs::create_dir_all(&self.carpeta_pruebas)?;
        let nombre: String = documento
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let ruta = self.carpeta_pruebas.join(format!("{nombre}.bin"));
        std::fs::write(&ruta, bytes)?;
        Ok(ruta)
    }
}
