//! Licencias de CHUPS POS, sin Tauri ni red.
//!
//! Una licencia es **permanente**: no vence, no tiene plan y no limita versiones ni
//! actualizaciones. Una vez activada en un equipo, vale para siempre en ese equipo.
//!
//! - El vendedor la firma con su clave privada Ed25519 (herramienta `tools/chups-licencias`).
//! - La app solo trae la clave pública ([`CLAVE_PUBLICA`]) y verifica la firma.
//! - Se entrega como texto base32 en bloques de 5 (`CHUPS-XXXXX-XXXXX-…`) o en un archivo
//!   `.chups-licencia` con ese mismo texto.
//! - Puede venir atada a un equipo (su [`Huella`]), pedida con el [`Solicitud`] que muestra la
//!   app, o sin equipo: entonces la app la ata al primer equipo donde se activa.
//!
//! El texto lleva los datos en binario compacto (no el JSON) para que sea corto; [`Licencia`] es
//! la forma legible, la que se guarda en el registro del vendedor y se muestra en la app.

pub mod base32;
pub mod huella;

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use huella::{huella_equipo, Huella, LARGO_HUELLA};

/// Prefijo del texto de una licencia. Es solo para reconocerla: no forma parte de los datos.
pub const PREFIJO: &str = "CHUPS";
/// Extensión del archivo de licencia.
pub const EXTENSION: &str = "chups-licencia";

const VERSION_FORMATO: u8 = 1;
const TIPO_SOLICITUD: u8 = 2;
/// Lo que se firma lleva este prefijo, para que una firma no sirva para otra cosa.
const DOMINIO: &[u8] = b"CHUPS-LICENCIA-v1\0";
const MAX_NEGOCIO: usize = 100;
const MAX_RUT: usize = 20;
pub const MAX_EQUIPOS: u8 = 50;

/// Clave pública del vendedor, incrustada en el binario. Sale de la variable de entorno
/// `CHUPS_CLAVE_PUBLICA` al compilar o, si no está, de `clave-publica.txt` (64 dígitos hex).
/// La privada nunca entra al repositorio.
pub const CLAVE_PUBLICA: [u8; 32] = hex32(match option_env!("CHUPS_CLAVE_PUBLICA") {
    Some(texto) => texto,
    None => include_str!("../clave-publica.txt"),
});

/// Hex de 64 dígitos -> 32 bytes, en tiempo de compilación (un error aquí no compila).
const fn hex32(texto: &str) -> [u8; 32] {
    let b = texto.as_bytes();
    let mut salida = [0u8; 32];
    let mut i = 0;
    let mut n = 0;
    let mut alto: Option<u8> = None;
    while i < b.len() {
        let valor = match b[i] {
            c @ b'0'..=b'9' => Some(c - b'0'),
            c @ b'a'..=b'f' => Some(c - b'a' + 10),
            c @ b'A'..=b'F' => Some(c - b'A' + 10),
            b' ' | b'\n' | b'\r' | b'\t' => None,
            _ => panic!("la clave pública debe ser hexadecimal"),
        };
        if let Some(v) = valor {
            match alto {
                None => alto = Some(v),
                Some(a) => {
                    assert!(n < 32, "la clave pública debe tener 64 dígitos hex");
                    salida[n] = a * 16 + v;
                    n += 1;
                    alto = None;
                }
            }
        }
        i += 1;
    }
    assert!(
        n == 32 && alto.is_none(),
        "la clave pública debe tener 64 dígitos hex"
    );
    salida
}

/// Una licencia, en su forma legible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Licencia {
    /// 16 dígitos hex, al azar.
    pub id: String,
    pub negocio: String,
    pub rut: Option<String>,
    /// "AAAA-MM-DD". Solo informativa: la licencia no vence.
    pub emitida_en: String,
    /// Cuántos equipos puede usar (1 por defecto).
    pub equipos: u8,
    /// Huella del equipo, o `None` si se ata al primer equipo donde se active.
    pub equipo: Option<Huella>,
}

impl Serialize for Huella {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.como_texto())
    }
}

impl<'de> Deserialize<'de> for Huella {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Huella, D::Error> {
        let texto = String::deserialize(d)?;
        Huella::desde_texto(&texto).ok_or_else(|| serde::de::Error::custom("huella inválida"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ErrorLicencia {
    #[error("El texto no es una licencia de CHUPS POS: revisa que esté completa ({0})")]
    Formato(String),
    #[error("La licencia no es válida: la firma no corresponde (está alterada o no la emitió el vendedor de CHUPS POS)")]
    Firma,
    #[error("Esta licencia es de otro equipo. Pide una licencia para este computador con su código de solicitud")]
    OtroEquipo,
    #[error("{0}")]
    Datos(String),
    #[error("El código de solicitud no es válido: revisa que esté completo")]
    Solicitud,
}

impl Licencia {
    /// Una licencia nueva, con id al azar y fecha de hoy (UTC).
    pub fn nueva(
        negocio: &str,
        rut: Option<&str>,
        equipos: u8,
        equipo: Option<Huella>,
    ) -> Result<Licencia, ErrorLicencia> {
        let mut id = [0u8; 8];
        getrandom::fill(&mut id).map_err(|e| ErrorLicencia::Datos(e.to_string()))?;
        let lic = Licencia {
            id: id.iter().map(|b| format!("{b:02x}")).collect(),
            negocio: negocio.trim().to_owned(),
            rut: rut
                .map(str::trim)
                .filter(|r| !r.is_empty())
                .map(str::to_owned),
            emitida_en: chrono::Utc::now()
                .date_naive()
                .format("%Y-%m-%d")
                .to_string(),
            equipos,
            equipo,
        };
        lic.validar()?;
        Ok(lic)
    }

    fn validar(&self) -> Result<(), ErrorLicencia> {
        let datos = |m: &str| Err(ErrorLicencia::Datos(m.to_owned()));
        if self.negocio.trim().is_empty() {
            return datos("Falta el nombre del negocio");
        }
        if self.negocio.len() > MAX_NEGOCIO {
            return datos("El nombre del negocio es demasiado largo (máximo 100 caracteres)");
        }
        if self.rut.as_ref().is_some_and(|r| r.len() > MAX_RUT) {
            return datos("El RUT es demasiado largo");
        }
        if self.equipos == 0 || self.equipos > MAX_EQUIPOS {
            return datos("La cantidad de equipos debe estar entre 1 y 50");
        }
        Ok(())
    }

    /// Los datos en binario, sin la firma.
    fn a_bytes(&self) -> Result<Vec<u8>, ErrorLicencia> {
        self.validar()?;
        let id = (0..8)
            .map(|i| u8::from_str_radix(self.id.get(i * 2..i * 2 + 2).unwrap_or(""), 16))
            .collect::<Result<Vec<u8>, _>>()
            .map_err(|_| ErrorLicencia::Datos("id inválido".into()))?;
        if self.id.len() != 16 {
            return Err(ErrorLicencia::Datos("id inválido".into()));
        }
        let fecha = chrono::NaiveDate::parse_from_str(&self.emitida_en, "%Y-%m-%d")
            .map_err(|_| ErrorLicencia::Datos("fecha de emisión inválida".into()))?;
        let dias = fecha
            .signed_duration_since(chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
            .num_days();
        let dias = u32::try_from(dias)
            .map_err(|_| ErrorLicencia::Datos("fecha de emisión inválida".into()))?;

        let mut b = Vec::with_capacity(64);
        b.push(VERSION_FORMATO);
        b.extend_from_slice(&id);
        b.extend_from_slice(&dias.to_be_bytes());
        b.push(self.equipos);
        b.push(u8::from(self.rut.is_some()) | (u8::from(self.equipo.is_some()) << 1));
        if let Some(h) = &self.equipo {
            b.extend_from_slice(&h.0);
        }
        b.push(self.negocio.len() as u8);
        b.extend_from_slice(self.negocio.as_bytes());
        if let Some(rut) = &self.rut {
            b.push(rut.len() as u8);
            b.extend_from_slice(rut.as_bytes());
        }
        Ok(b)
    }

    /// Lee los datos binarios; devuelve la licencia y cuántos bytes ocupó.
    fn desde_bytes(b: &[u8]) -> Result<(Licencia, usize), ErrorLicencia> {
        let corta = || ErrorLicencia::Formato("faltan caracteres".into());
        let mut i = 0;
        let mut tomar = |n: usize| -> Result<&[u8], ErrorLicencia> {
            let parte = b.get(i..i + n).ok_or_else(corta)?;
            i += n;
            Ok(parte)
        };
        let version = tomar(1)?[0];
        if version != VERSION_FORMATO {
            return Err(ErrorLicencia::Formato(format!(
                "versión de formato {version} desconocida"
            )));
        }
        let id: String = tomar(8)?.iter().map(|x| format!("{x:02x}")).collect();
        let dias = u32::from_be_bytes(tomar(4)?.try_into().unwrap());
        let equipos = tomar(1)?[0];
        let banderas = tomar(1)?[0];
        let equipo = if banderas & 2 != 0 {
            Some(Huella(tomar(LARGO_HUELLA)?.try_into().unwrap()))
        } else {
            None
        };
        let mut texto = |que: &str| -> Result<String, ErrorLicencia> {
            let largo = tomar(1)?[0] as usize;
            String::from_utf8(tomar(largo)?.to_vec())
                .map_err(|_| ErrorLicencia::Formato(format!("{que} ilegible")))
        };
        let negocio = texto("nombre del negocio")?;
        let rut = if banderas & 1 != 0 {
            Some(texto("RUT")?)
        } else {
            None
        };
        let fecha = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
            + chrono::Days::new(u64::from(dias));
        let lic = Licencia {
            id,
            negocio,
            rut,
            emitida_en: fecha.format("%Y-%m-%d").to_string(),
            equipos,
            equipo,
        };
        Ok((lic, i))
    }
}

/// Firma la licencia con la clave privada del vendedor y la deja como texto `CHUPS-XXXXX-…`.
pub fn firmar(lic: &Licencia, clave_privada: &[u8; 32]) -> Result<String, ErrorLicencia> {
    let datos = lic.a_bytes()?;
    let clave = SigningKey::from_bytes(clave_privada);
    let firma = clave.sign(&mensaje(&datos));
    let mut todo = datos;
    todo.extend_from_slice(&firma.to_bytes());
    Ok(format!(
        "{PREFIJO}-{}",
        base32::en_bloques(&base32::codificar(&todo))
    ))
}

/// Lee una licencia (texto o contenido de un archivo `.chups-licencia`) y verifica su firma con
/// `clave_publica`. No revisa el equipo: eso lo hace [`validar_equipo`].
pub fn leer(texto: &str, clave_publica: &[u8; 32]) -> Result<Licencia, ErrorLicencia> {
    // El archivo puede traer líneas de comentario (las que empiezan con #).
    let codigo: String = texto
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect();
    let normal = base32::normalizar(&codigo);
    let sin_prefijo = normal.strip_prefix(PREFIJO).unwrap_or(&normal);
    if sin_prefijo.is_empty() {
        return Err(ErrorLicencia::Formato("está vacía".into()));
    }
    let bytes = base32::decodificar(sin_prefijo)
        .ok_or_else(|| ErrorLicencia::Formato("tiene caracteres que no corresponden".into()))?;
    let (lic, largo) = Licencia::desde_bytes(&bytes)?;
    let resto = &bytes[largo..];
    let firma: [u8; 64] = resto
        .try_into()
        .map_err(|_| ErrorLicencia::Formato("le faltan o le sobran caracteres".into()))?;
    let clave = VerifyingKey::from_bytes(clave_publica).map_err(|_| ErrorLicencia::Firma)?;
    clave
        .verify(&mensaje(&bytes[..largo]), &Signature::from_bytes(&firma))
        .map_err(|_| ErrorLicencia::Firma)?;
    Ok(lic)
}

fn mensaje(datos: &[u8]) -> Vec<u8> {
    let mut m = DOMINIO.to_vec();
    m.extend_from_slice(datos);
    m
}

/// Revisa que la licencia sirva en este equipo y devuelve la huella a la que queda atada.
///
/// - Con equipo: tiene que ser este.
/// - Sin equipo: se ata a este equipo; si ya estaba atada (`atada`), tiene que ser a este.
pub fn validar_equipo(
    lic: &Licencia,
    actual: &Huella,
    atada: Option<&Huella>,
) -> Result<Huella, ErrorLicencia> {
    match (&lic.equipo, atada) {
        (Some(h), _) if h == actual => Ok(*h),
        (Some(_), _) => Err(ErrorLicencia::OtroEquipo),
        (None, Some(a)) if a == actual => Ok(*a),
        (None, Some(_)) => Err(ErrorLicencia::OtroEquipo),
        (None, None) => Ok(*actual),
    }
}

/// Una clave privada nueva (32 bytes al azar).
pub fn generar_clave_privada() -> Result<[u8; 32], String> {
    let mut semilla = [0u8; 32];
    getrandom::fill(&mut semilla).map_err(|e| e.to_string())?;
    Ok(semilla)
}

pub fn clave_publica_de(clave_privada: &[u8; 32]) -> [u8; 32] {
    SigningKey::from_bytes(clave_privada)
        .verifying_key()
        .to_bytes()
}

/// Lo que la app muestra para pedir una licencia atada a este equipo: la huella y la versión.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Solicitud {
    pub huella: Huella,
    pub version: (u8, u8, u8),
}

impl Solicitud {
    /// `version` como "1.0.0" (lo que siga a un guion se ignora).
    pub fn nueva(huella: Huella, version: &str) -> Solicitud {
        let mut partes = version
            .split(['-', '+'])
            .next()
            .unwrap_or("")
            .split('.')
            .map(|p| p.parse::<u8>().unwrap_or(0));
        let mut siguiente = || partes.next().unwrap_or(0);
        Solicitud {
            huella,
            version: (siguiente(), siguiente(), siguiente()),
        }
    }

    /// 26 caracteres en bloques de 5, con una suma de control para detectar errores de tipeo.
    pub fn como_texto(&self) -> String {
        let mut b = vec![TIPO_SOLICITUD];
        b.extend_from_slice(&self.huella.0);
        b.extend_from_slice(&[self.version.0, self.version.1, self.version.2]);
        let suma = Sha256::digest(&b);
        b.extend_from_slice(&suma[..2]);
        base32::en_bloques(&base32::codificar(&b))
    }

    pub fn desde_texto(texto: &str) -> Result<Solicitud, ErrorLicencia> {
        let b = base32::decodificar(texto).ok_or(ErrorLicencia::Solicitud)?;
        if b.len() != 16 || b[0] != TIPO_SOLICITUD {
            return Err(ErrorLicencia::Solicitud);
        }
        if Sha256::digest(&b[..14])[..2] != b[14..16] {
            return Err(ErrorLicencia::Solicitud);
        }
        Ok(Solicitud {
            huella: Huella(b[1..11].try_into().unwrap()),
            version: (b[11], b[12], b[13]),
        })
    }

    pub fn version_texto(&self) -> String {
        format!("{}.{}.{}", self.version.0, self.version.1, self.version.2)
    }
}
