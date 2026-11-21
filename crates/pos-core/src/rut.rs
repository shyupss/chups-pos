//! RUT chileno: valida el dígito verificador y lo deja como 12.345.678-5.

use crate::error::validacion;
use crate::Resultado;

/// Dígito verificador por módulo 11: los dígitos del cuerpo de derecha a izquierda
/// se multiplican por 2, 3, 4, 5, 6, 7, 2, 3…; 11 - (suma % 11), donde 11 es 0 y 10 es K.
pub fn digito_verificador(cuerpo: u32) -> char {
    let mut suma = 0;
    let mut factor = 2;
    let mut resto = cuerpo;
    while resto > 0 {
        suma += (resto % 10) * factor;
        resto /= 10;
        factor = if factor == 7 { 2 } else { factor + 1 };
    }
    match 11 - suma % 11 {
        11 => '0',
        10 => 'K',
        d => char::from_digit(d, 10).expect("es un dígito"),
    }
}

/// Acepta "12.345.678-5", "12345678-5", "123456785" o "12 345 678 k" y devuelve "12.345.678-5".
pub fn normalizar(texto: &str) -> Resultado<String> {
    let limpio: String = texto
        .chars()
        .filter(|c| !matches!(c, '.' | '-' | ' '))
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let mal = || validacion("El RUT no es válido: revisa los números y el dígito verificador");
    if limpio.len() < 2 || !limpio.is_ascii() {
        return mal();
    }
    let (cuerpo, dv) = limpio.split_at(limpio.len() - 1);
    if cuerpo.is_empty() || cuerpo.len() > 9 || !cuerpo.chars().all(|c| c.is_ascii_digit()) {
        return mal();
    }
    let Ok(numero) = cuerpo.parse::<u32>() else {
        return mal();
    };
    if numero == 0 || !dv.starts_with(digito_verificador(numero)) {
        return mal();
    }
    Ok(format!("{}-{dv}", con_puntos(numero)))
}

fn con_puntos(n: u32) -> String {
    let digitos = n.to_string();
    let mut salida = String::with_capacity(digitos.len() + 3);
    for (i, c) in digitos.chars().enumerate() {
        if i > 0 && (digitos.len() - i).is_multiple_of(3) {
            salida.push('.');
        }
        salida.push(c);
    }
    salida
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calcula_el_digito() {
        assert_eq!(digito_verificador(12_345_678), '5');
        assert_eq!(digito_verificador(11_111_111), '1');
        assert_eq!(digito_verificador(10_000_013), 'K');
        assert_eq!(digito_verificador(76_086_428), '5');
    }
}
