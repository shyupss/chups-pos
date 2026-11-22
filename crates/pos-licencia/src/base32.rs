//! Base32 (alfabeto RFC 4648, sin relleno) en bloques de 5 caracteres, para copiar o dictar.
//!
//! Al leer se aceptan minúsculas, guiones, espacios y saltos de línea, y los caracteres que se
//! confunden al dictar o escribir a mano: 0 se lee como O, 1 como I y 8 como B (el alfabeto no
//! tiene 0, 1, 8 ni 9).

const ALFABETO: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Bytes -> base32 sin relleno ni separadores.
pub fn codificar(bytes: &[u8]) -> String {
    let mut salida = String::with_capacity(bytes.len() * 8 / 5 + 1);
    let mut acumulado: u32 = 0;
    let mut bits = 0;
    for &b in bytes {
        acumulado = (acumulado << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            salida.push(ALFABETO[((acumulado >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        salida.push(ALFABETO[((acumulado << (5 - bits)) & 31) as usize] as char);
    }
    salida
}

/// Base32 -> bytes. `None` si trae un carácter que no es del alfabeto (después de normalizar).
/// Los bits que sobran al final (menos de 8) se descartan.
pub fn decodificar(texto: &str) -> Option<Vec<u8>> {
    let mut salida = Vec::with_capacity(texto.len() * 5 / 8);
    let mut acumulado: u32 = 0;
    let mut bits = 0;
    for c in normalizar(texto).bytes() {
        let valor = ALFABETO.iter().position(|&a| a == c)? as u32;
        acumulado = (acumulado << 5) | valor;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            salida.push((acumulado >> bits) as u8);
            acumulado &= (1 << bits) - 1;
        }
    }
    Some(salida)
}

/// Mayúsculas, sin separadores y con los caracteres confundibles corregidos.
pub fn normalizar(texto: &str) -> String {
    texto
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-' && *c != '_' && *c != '.')
        .map(|c| match c.to_ascii_uppercase() {
            '0' => 'O',
            '1' => 'I',
            '8' => 'B',
            otro => otro,
        })
        .collect()
}

/// "ABCDEFGHIJ" -> "ABCDE-FGHIJ".
pub fn en_bloques(texto: &str) -> String {
    let caracteres: Vec<char> = texto.chars().collect();
    caracteres
        .chunks(5)
        .map(|b| b.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta_de_todos_los_largos() {
        for largo in 0..40 {
            let bytes: Vec<u8> = (0..largo).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(
                decodificar(&codificar(&bytes)).unwrap(),
                bytes,
                "largo {largo}"
            );
        }
    }

    #[test]
    fn acepta_guiones_minusculas_y_confundibles() {
        let bytes = b"hola licencia";
        let texto = en_bloques(&codificar(bytes));
        let escrito = texto.to_lowercase().replace('o', "0").replace('i', "1");
        assert_eq!(decodificar(&escrito).unwrap(), bytes);
        assert_eq!(decodificar(&format!(" {texto}\n")).unwrap(), bytes);
    }

    #[test]
    fn rechaza_caracteres_ajenos() {
        assert!(decodificar("ABC9D").is_none());
        assert!(decodificar("AB$CD").is_none());
    }
}
