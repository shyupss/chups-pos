//! Formato chileno para la boleta: $12.345, 0,532 kg, N° 000123.

use chrono::{DateTime, Local};

use crate::repositorios::productos::Unidad;

/// 12345 -> "12.345"
fn miles(n: u64) -> String {
    let digitos = n.to_string();
    let mut salida = String::with_capacity(digitos.len() + digitos.len() / 3);
    for (i, c) in digitos.chars().enumerate() {
        if i > 0 && (digitos.len() - i).is_multiple_of(3) {
            salida.push('.');
        }
        salida.push(c);
    }
    salida
}

/// 12345 -> "$12.345"; -4 -> "-$4"
pub fn pesos(monto: i64) -> String {
    let signo = if monto < 0 { "-" } else { "" };
    format!("{signo}${}", miles(monto.unsigned_abs()))
}

/// Ajuste con signo explícito: 4 -> "+$4", -4 -> "-$4".
pub fn pesos_con_signo(monto: i64) -> String {
    if monto > 0 {
        format!("+{}", pesos(monto))
    } else {
        pesos(monto)
    }
}

/// Milésimas con tres decimales: 532 -> "0,532"; 1500 -> "1,500".
fn tres_decimales(mil: i64) -> String {
    let signo = if mil < 0 { "-" } else { "" };
    let abs = mil.unsigned_abs();
    format!("{signo}{},{:03}", miles(abs / 1000), abs % 1000)
}

/// Cantidad vendida: (3000, unidad) -> "3"; (532, kg) -> "0,532 kg".
pub fn cantidad(cantidad_mil: i64, unidad: Unidad) -> String {
    match unidad {
        Unidad::Unidad if cantidad_mil % 1000 == 0 => miles(cantidad_mil.unsigned_abs() / 1000),
        Unidad::Unidad => tres_decimales(cantidad_mil),
        Unidad::Kg => format!("{} kg", tres_decimales(cantidad_mil)),
    }
}

/// Detalle bajo el nombre: "3 x $1.190" o "0,532 kg x $2.300/kg".
pub fn detalle(cantidad_mil: i64, unidad: Unidad, precio_unitario: i64) -> String {
    let por = if unidad == Unidad::Kg { "/kg" } else { "" };
    format!(
        "{} x {}{por}",
        cantidad(cantidad_mil, unidad),
        pesos(precio_unitario)
    )
}

/// 123 -> "000123"
pub fn folio(folio: i64) -> String {
    format!("{folio:06}")
}

/// La base guarda UTC ("2026-11-21T17:05:00.000Z"); la boleta muestra la hora del equipo.
pub fn fecha_local(utc: &str) -> String {
    DateTime::parse_from_rfc3339(utc)
        .map(|f| f.with_timezone(&Local).format("%d-%m-%Y %H:%M").to_string())
        .unwrap_or_else(|_| utc.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn montos_y_cantidades_en_formato_chileno() {
        assert_eq!(pesos(0), "$0");
        assert_eq!(pesos(990), "$990");
        assert_eq!(pesos(12_345), "$12.345");
        assert_eq!(pesos(1_234_567), "$1.234.567");
        assert_eq!(pesos(-4), "-$4");
        assert_eq!(pesos_con_signo(4), "+$4");
        assert_eq!(cantidad(3000, Unidad::Unidad), "3");
        assert_eq!(cantidad(1_500_000, Unidad::Unidad), "1.500");
        assert_eq!(cantidad(532, Unidad::Kg), "0,532 kg");
        assert_eq!(cantidad(12_500, Unidad::Kg), "12,500 kg");
        assert_eq!(detalle(532, Unidad::Kg, 2300), "0,532 kg x $2.300/kg");
        assert_eq!(detalle(2000, Unidad::Unidad, 1190), "2 x $1.190");
        assert_eq!(folio(123), "000123");
    }

    #[test]
    fn fecha_invalida_se_muestra_tal_cual() {
        assert_eq!(fecha_local("ayer"), "ayer");
        // Formato correcto, sea cual sea la zona horaria del equipo.
        let f = fecha_local("2026-11-21T17:05:00.000Z");
        assert_eq!(f.len(), "21-11-2026 17:05".len());
        assert_eq!(&f[2..3], "-");
    }
}
