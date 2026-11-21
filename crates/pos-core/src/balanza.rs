//! Códigos de las etiquetas de balanza (EAN-13 de uso interno, prefijos 20 a 29).
//!
//! Formato: `PP CCCCC VVVVV D`
//! - `PP`: prefijo (20–29, configurable)
//! - `CCCCC`: código del producto (4 a 6 dígitos, configurable)
//! - `VVVVV`: peso en gramos o precio en pesos (los dígitos que quedan)
//! - `D`: dígito verificador EAN-13
//!
//! Ejemplo con código de 5 dígitos y peso: `2012345005327` → producto 12345, 532 g.

use crate::error::validacion;
use crate::servicios::config::{Config, FormatoBalanza};
use crate::Resultado;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReglasBalanza {
    pub prefijos: Vec<u8>,
    pub formato: FormatoBalanza,
    /// Dígitos del código de producto (4 a 6).
    pub largo_codigo: usize,
}

impl ReglasBalanza {
    pub fn desde(cfg: &Config) -> Self {
        ReglasBalanza {
            prefijos: cfg.prefijos_balanza(),
            formato: cfg.balanza_formato,
            largo_codigo: usize::from(cfg.balanza_largo_codigo),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValorBalanza {
    Gramos(i64),
    Pesos(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodigoBalanza {
    pub codigo_producto: String,
    pub valor: ValorBalanza,
}

/// Dígito verificador EAN-13 de los primeros 12 dígitos: pesos 1 y 3 alternados.
pub fn digito_verificador(primeros12: &[u8]) -> u8 {
    let suma: u32 = primeros12
        .iter()
        .enumerate()
        .map(|(i, d)| u32::from(*d) * if i % 2 == 0 { 1 } else { 3 })
        .sum();
    ((10 - suma % 10) % 10) as u8
}

/// Analiza un código escaneado.
/// - `Ok(None)`: no es una etiqueta de balanza (otro largo, otro prefijo o letras).
/// - `Err`: tiene forma de etiqueta pero el dígito verificador no calza (lectura dañada).
pub fn analizar(codigo: &str, reglas: &ReglasBalanza) -> Resultado<Option<CodigoBalanza>> {
    let codigo = codigo.trim();
    if codigo.len() != 13 || !codigo.bytes().all(|b| b.is_ascii_digit()) {
        return Ok(None);
    }
    let digitos: Vec<u8> = codigo.bytes().map(|b| b - b'0').collect();
    let prefijo = digitos[0] * 10 + digitos[1];
    if !reglas.prefijos.contains(&prefijo) || !(4..=6).contains(&reglas.largo_codigo) {
        return Ok(None);
    }
    if digitos[12] != digito_verificador(&digitos[..12]) {
        return validacion(format!(
            "El código de balanza {codigo} tiene el dígito verificador incorrecto: vuelve a escanear la etiqueta"
        ));
    }
    let fin_codigo = 2 + reglas.largo_codigo;
    let valor: i64 = codigo[fin_codigo..12].parse().expect("son dígitos");
    Ok(Some(CodigoBalanza {
        codigo_producto: codigo[2..fin_codigo].to_owned(),
        valor: match reglas.formato {
            FormatoBalanza::Peso => ValorBalanza::Gramos(valor),
            FormatoBalanza::Precio => ValorBalanza::Pesos(valor),
        },
    }))
}

/// Gramos que corresponden a un precio de etiqueta, redondeados al gramo más cercano.
/// `None` si el producto no tiene precio por kilo.
pub fn gramos_desde_precio(precio_etiqueta: i64, precio_kg: i64) -> Option<i64> {
    if precio_kg <= 0 {
        return None;
    }
    precio_etiqueta
        .checked_mul(1000)?
        .checked_add(precio_kg / 2)
        .map(|v| v / precio_kg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calcula_el_digito_ean13() {
        let d = |s: &str| digito_verificador(&s.bytes().map(|b| b - b'0').collect::<Vec<_>>());
        assert_eq!(d("780161000119"), 6); // 7801610001196
        assert_eq!(d("201234500532"), 7); // 2012345005327
        assert_eq!(d("200000000001"), 5);
    }
}
