//! Leer planillas para importar: .xlsx, .xls y .ods con calamine (la primera hoja) y .csv con
//! un lector propio (calamine no lee CSV), que detecta el separador (; , o tabulación) y la
//! codificación (UTF-8 o Windows-1252, la de un CSV guardado por Excel en español).

use std::hash::{Hash, Hasher};
use std::path::Path;

use calamine::{open_workbook_auto, Data, Reader};

use crate::error::validacion;
use crate::{Error, Resultado};

/// Las planillas más grandes que se aceptan: más filas suele ser un archivo equivocado.
pub const MAX_FILAS: usize = 100_000;

/// Una celda tal como venía: los números de Excel llegan como número y los de un CSV como texto.
#[derive(Debug, Clone, PartialEq)]
pub enum Celda {
    Vacia,
    Texto(String),
    Numero(f64),
}

impl Celda {
    /// El texto de la celda, sin espacios a los lados (los números sin ".0" sobrante).
    pub fn texto(&self) -> String {
        match self {
            Celda::Vacia => String::new(),
            Celda::Texto(t) => t.trim().to_owned(),
            Celda::Numero(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{}", *n as i64),
            Celda::Numero(n) => n.to_string(),
        }
    }

    pub fn vacia(&self) -> bool {
        match self {
            Celda::Vacia => true,
            Celda::Texto(t) => t.trim().is_empty(),
            Celda::Numero(_) => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Planilla {
    pub encabezados: Vec<String>,
    /// Las filas de datos (sin la de encabezados), todas del mismo largo que los encabezados.
    pub filas: Vec<Vec<Celda>>,
    /// Número de la fila de encabezados en la planilla (1 = la primera), para los mensajes.
    pub fila_encabezados: usize,
    /// Huella del archivo: si cambia entre la vista previa y la importación, se avisa.
    pub huella: String,
}

impl Planilla {
    /// El número de fila en la planilla (como lo ve el usuario) de la fila de datos `i`.
    pub fn numero_fila(&self, i: usize) -> usize {
        self.fila_encabezados + 1 + i
    }
}

fn huella(bytes: &[u8]) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    format!("{:016x}-{}", h.finish(), bytes.len())
}

/// Lee la primera hoja (o el CSV). La fila de encabezados es la primera con al menos dos celdas
/// (así una planilla exportada por la app, con su título arriba, se vuelve a importar); si
/// ninguna de las primeras tiene dos, la primera que tiene algo.
pub fn leer(ruta: &Path) -> Resultado<Planilla> {
    let bytes = std::fs::read(ruta)
        .map_err(|e| Error::Validacion(format!("No se pudo leer el archivo: {e}")))?;
    let extension = ruta
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let filas = match extension.as_str() {
        "csv" | "txt" => leer_csv(&bytes),
        "xlsx" | "xlsm" | "xls" | "ods" => leer_libro(ruta)?,
        _ => return validacion("El archivo debe ser una planilla .xlsx, .xls, .ods o .csv"),
    };
    armar(filas, huella(&bytes))
}

fn armar(filas: Vec<Vec<Celda>>, huella: String) -> Resultado<Planilla> {
    let llenas = |f: &Vec<Celda>| f.iter().filter(|c| !c.vacia()).count();
    let Some(primera) = filas.iter().position(|f| llenas(f) > 0) else {
        return validacion("La planilla está vacía");
    };
    let inicio = filas
        .iter()
        .enumerate()
        .skip(primera)
        .take(20)
        .find(|(_, f)| llenas(f) >= 2)
        .map_or(primera, |(i, _)| i);
    let encabezados: Vec<String> = filas[inicio].iter().map(Celda::texto).collect();
    let ancho = encabezados.len();
    let datos: Vec<Vec<Celda>> = filas
        .into_iter()
        .skip(inicio + 1)
        .map(|mut f| {
            f.resize(ancho, Celda::Vacia);
            f.truncate(ancho);
            f
        })
        .collect();
    // Las filas vacías del final no cuentan (Excel suele dejarlas).
    let ultima = datos
        .iter()
        .rposition(|f| f.iter().any(|c| !c.vacia()))
        .map_or(0, |i| i + 1);
    let mut datos = datos;
    datos.truncate(ultima);
    if datos.len() > MAX_FILAS {
        return validacion(format!(
            "La planilla tiene {} filas: el máximo es {MAX_FILAS}",
            datos.len()
        ));
    }
    Ok(Planilla {
        encabezados,
        filas: datos,
        fila_encabezados: inicio + 1,
        huella,
    })
}

fn leer_libro(ruta: &Path) -> Resultado<Vec<Vec<Celda>>> {
    let mut libro = open_workbook_auto(ruta).map_err(|e| {
        Error::Validacion(format!(
            "No se pudo abrir la planilla (¿está dañada o protegida?): {e}"
        ))
    })?;
    let hoja = libro
        .worksheet_range_at(0)
        .ok_or_else(|| Error::Validacion("La planilla no tiene hojas".into()))?
        .map_err(|e| Error::Validacion(format!("No se pudo leer la primera hoja: {e}")))?;
    Ok(hoja
        .rows()
        .map(|fila| {
            fila.iter()
                .map(|c| match c {
                    Data::Empty | Data::Error(_) => Celda::Vacia,
                    Data::String(s) => Celda::Texto(s.clone()),
                    Data::Int(n) => Celda::Numero(*n as f64),
                    Data::Float(n) => Celda::Numero(*n),
                    Data::Bool(b) => Celda::Texto(if *b { "sí" } else { "no" }.into()),
                    Data::DateTime(d) => Celda::Numero(d.as_f64()),
                    Data::DateTimeIso(s) | Data::DurationIso(s) => Celda::Texto(s.clone()),
                })
                .collect()
        })
        .collect())
}

/// Texto del CSV: UTF-8 (con o sin BOM) o, si no lo es, Windows-1252.
fn decodificar(bytes: &[u8]) -> String {
    let sin_bom = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(sin_bom) {
        Ok(t) => t.to_owned(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(sin_bom).0.into_owned(),
    }
}

/// El separador más frecuente fuera de comillas en la primera línea.
fn separador(texto: &str) -> char {
    let primera = texto.lines().next().unwrap_or_default();
    let mut en_comillas = false;
    let mut cuentas = [(';', 0), (',', 0), ('\t', 0)];
    for c in primera.chars() {
        if c == '"' {
            en_comillas = !en_comillas;
        } else if !en_comillas {
            for (s, n) in &mut cuentas {
                if c == *s {
                    *n += 1;
                }
            }
        }
    }
    cuentas
        .iter()
        .max_by_key(|(_, n)| *n)
        .filter(|(_, n)| *n > 0)
        .map_or(';', |(s, _)| *s)
}

/// CSV con comillas dobles ("" dentro de comillas es una comilla) y saltos de línea en campos.
fn leer_csv(bytes: &[u8]) -> Vec<Vec<Celda>> {
    let texto = decodificar(bytes);
    let sep = separador(&texto);
    let mut filas = Vec::new();
    let mut fila = Vec::new();
    let mut campo = String::new();
    let mut en_comillas = false;
    let mut chars = texto.chars().peekable();
    let cerrar = |campo: &mut String, fila: &mut Vec<Celda>| {
        let t = std::mem::take(campo);
        fila.push(if t.trim().is_empty() {
            Celda::Vacia
        } else {
            Celda::Texto(t)
        });
    };
    while let Some(c) = chars.next() {
        if en_comillas {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    campo.push('"');
                    chars.next();
                } else {
                    en_comillas = false;
                }
            } else {
                campo.push(c);
            }
        } else if c == '"' {
            en_comillas = true;
        } else if c == sep {
            cerrar(&mut campo, &mut fila);
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            cerrar(&mut campo, &mut fila);
            filas.push(std::mem::take(&mut fila));
        } else {
            campo.push(c);
        }
    }
    if !campo.is_empty() || !fila.is_empty() {
        cerrar(&mut campo, &mut fila);
        filas.push(fila);
    }
    filas
}

/// Lee un CSV ya en memoria (para las pruebas).
pub fn leer_csv_bytes(bytes: &[u8]) -> Resultado<Planilla> {
    armar(leer_csv(bytes), huella(bytes))
}

// --- Números escritos a mano ---------------------------------------------------------------

/// Pesos enteros: acepta "1190", "$1.190", "1.190,00", "$ 1 190" y números de Excel sin decimales.
pub fn pesos(c: &Celda) -> Result<Option<i64>, String> {
    match c {
        Celda::Vacia => Ok(None),
        Celda::Numero(n) => {
            if n.fract() != 0.0 {
                Err(format!(
                    "{n} tiene decimales: los montos van en pesos enteros"
                ))
            } else if n.abs() > 1e12 {
                Err(format!("{n} es demasiado grande"))
            } else {
                Ok(Some(*n as i64))
            }
        }
        Celda::Texto(t) => {
            let t: String = t
                .chars()
                .filter(|c| !c.is_whitespace() && *c != '$')
                .collect();
            if t.is_empty() {
                return Ok(None);
            }
            match milesimas(&t) {
                Some(m) if m % 1000 == 0 => Ok(Some(m / 1000)),
                Some(_) => Err(format!(
                    "«{t}» tiene decimales: los montos van en pesos enteros"
                )),
                None => Err(format!("«{t}» no es un monto")),
            }
        }
    }
}

/// Una cantidad en milésimas: "12", "1,5" (coma decimal), "1.500" (punto de miles) o "0.532"
/// (punto decimal cuando no parece de miles). Los números de Excel se toman tal cual.
pub fn cantidad(c: &Celda) -> Result<Option<i64>, String> {
    match c {
        Celda::Vacia => Ok(None),
        Celda::Numero(n) => {
            let m = (n * 1000.0).round();
            if m.abs() > 1e15 {
                Err(format!("{n} es demasiado grande"))
            } else {
                Ok(Some(m as i64))
            }
        }
        Celda::Texto(t) => {
            let t: String = t.chars().filter(|c| !c.is_whitespace()).collect();
            let t = t.trim_end_matches(|c: char| c.is_alphabetic()).to_owned();
            if t.is_empty() {
                return Ok(None);
            }
            milesimas(&t)
                .map(Some)
                .ok_or_else(|| format!("«{t}» no es una cantidad"))
        }
    }
}

/// Convierte un número escrito a milésimas, sin pasar por decimales binarios.
fn milesimas(t: &str) -> Option<i64> {
    let (negativo, t) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t),
    };
    let (entera, decimal) = if let Some((e, d)) = t.split_once(',') {
        // Coma decimal: los puntos de la parte entera son de miles.
        (e.replace('.', ""), d.to_owned())
    } else if t.matches('.').count() > 1 || es_de_miles(t) {
        (t.replace('.', ""), String::new())
    } else if let Some((e, d)) = t.split_once('.') {
        (e.to_owned(), d.to_owned())
    } else {
        (t.to_owned(), String::new())
    };
    if entera.is_empty() && decimal.is_empty() {
        return None;
    }
    if !entera.chars().all(|c| c.is_ascii_digit()) || !decimal.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let decimal = decimal.trim_end_matches('0');
    if decimal.len() > 3 || entera.len() > 15 {
        return None;
    }
    let e: i64 = if entera.is_empty() {
        0
    } else {
        entera.parse().ok()?
    };
    let d: i64 = format!("{decimal:0<3}").parse().ok()?;
    let v = e.checked_mul(1000)?.checked_add(d)?;
    Some(if negativo { -v } else { v })
}

/// "1.500" o "12.345.678": grupos de tres después del primer punto. "0.532" no (un número con
/// separador de miles no empieza en 0).
fn es_de_miles(t: &str) -> bool {
    let partes: Vec<&str> = t.split('.').collect();
    partes.len() > 1
        && !partes[0].is_empty()
        && !partes[0].starts_with('0')
        && partes[0].len() <= 3
        && partes[1..].iter().all(|p| p.len() == 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Celda {
        Celda::Texto(s.into())
    }

    #[test]
    fn montos_y_cantidades_escritos_de_varias_formas() {
        assert_eq!(pesos(&t("1190")), Ok(Some(1190)));
        assert_eq!(pesos(&t("$1.190")), Ok(Some(1190)));
        assert_eq!(pesos(&t("$ 1.190,00")), Ok(Some(1190)));
        assert_eq!(pesos(&Celda::Numero(2490.0)), Ok(Some(2490)));
        assert!(pesos(&t("12,5")).is_err());
        assert!(pesos(&Celda::Numero(12.5)).is_err());
        assert!(pesos(&t("abc")).is_err());
        assert_eq!(pesos(&t("  ")), Ok(None));

        assert_eq!(cantidad(&t("12")), Ok(Some(12_000)));
        assert_eq!(cantidad(&t("1,5")), Ok(Some(1500)));
        assert_eq!(cantidad(&t("1.500")), Ok(Some(1_500_000)));
        assert_eq!(cantidad(&t("0.532")), Ok(Some(532)));
        assert_eq!(cantidad(&t("2,5 kg")), Ok(Some(2500)));
        assert_eq!(cantidad(&Celda::Numero(0.532)), Ok(Some(532)));
        assert!(cantidad(&t("1,2345")).is_err());
    }

    #[test]
    fn csv_con_punto_y_coma_comillas_y_windows_1252() {
        let bytes = b"C\xf3digo;Nombre;Precio\r\n780;\"Leche \"\"Colun\"\"; 1 L\";1.190\r\n\r\n;Pan;990\r\n";
        let p = leer_csv_bytes(bytes).unwrap();
        assert_eq!(p.encabezados, ["Código", "Nombre", "Precio"]);
        assert_eq!(p.filas.len(), 3);
        assert_eq!(p.filas[0][1].texto(), "Leche \"Colun\"; 1 L");
        assert!(p.filas[1].iter().all(Celda::vacia));
        assert_eq!(p.filas[2][1].texto(), "Pan");
        assert_eq!(p.numero_fila(2), 4);

        let coma = leer_csv_bytes("\u{feff}nombre,precio\nArroz,1590\n".as_bytes()).unwrap();
        assert_eq!(coma.encabezados, ["nombre", "precio"]);
        assert_eq!(coma.filas[0][1].texto(), "1590");
    }
}
