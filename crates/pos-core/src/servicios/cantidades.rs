//! Reglas de las cantidades en milésimas: 1 unidad = 1000, 0,532 kg = 532.

use crate::error::validacion;
use crate::repositorios::productos::Unidad;
use crate::Resultado;

pub const MIL: i64 = 1000;
/// Peso máximo de una línea: 100 kg.
pub const MAX_KG_MIL: i64 = 100 * MIL;
/// Unidades máximas de una línea.
pub const MAX_UNIDADES_MIL: i64 = 100_000 * MIL;

/// Subtotal de una línea redondeado al peso más cercano: (precio × cantidad_mil + 500) / 1000.
/// `None` si el cálculo se desborda.
pub fn subtotal(precio: i64, cantidad_mil: i64) -> Option<i64> {
    precio
        .checked_mul(cantidad_mil)?
        .checked_add(MIL / 2)
        .map(|v| v / MIL)
}

/// Valida la cantidad vendida de un producto según cómo se vende.
pub fn validar_venta(unidad: Unidad, nombre: &str, cantidad_mil: i64) -> Resultado<()> {
    match unidad {
        Unidad::Unidad if cantidad_mil % MIL != 0 => validacion(format!(
            "{nombre} se vende por unidad: la cantidad debe ser un número entero"
        )),
        Unidad::Unidad if !(MIL..=MAX_UNIDADES_MIL).contains(&cantidad_mil) => validacion(format!(
            "La cantidad de {nombre} debe estar entre 1 y 100.000 unidades"
        )),
        Unidad::Kg if !(1..=MAX_KG_MIL).contains(&cantidad_mil) => {
            validacion(format!("El peso de {nombre} debe estar entre 1 g y 100 kg"))
        }
        _ => Ok(()),
    }
}

/// Valida un stock que se ingresa a mano (inicial o mínimo): no negativo y, en productos
/// por unidad, un número entero de unidades.
pub fn validar_stock(unidad: Unidad, cantidad_mil: i64, que: &str) -> Resultado<()> {
    if cantidad_mil < 0 {
        return validacion(format!("El {que} no puede ser negativo"));
    }
    if unidad == Unidad::Unidad && cantidad_mil % MIL != 0 {
        return validacion(format!("El {que} debe ser un número entero de unidades"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtotal_redondea_al_peso_mas_cercano() {
        assert_eq!(subtotal(2300, 532), Some(1224)); // 1223,6
        assert_eq!(subtotal(1000, 500), Some(500));
        assert_eq!(subtotal(1, 500), Some(1)); // 0,5 sube
        assert_eq!(subtotal(1, 499), Some(0));
        assert_eq!(subtotal(1190, 3000), Some(3570));
        assert_eq!(subtotal(i64::MAX, 2), None);
    }

    #[test]
    fn valida_segun_unidad() {
        assert!(validar_venta(Unidad::Unidad, "Pan", 3000).is_ok());
        assert!(validar_venta(Unidad::Unidad, "Pan", 1500).is_err());
        assert!(validar_venta(Unidad::Kg, "Queso", 1).is_ok());
        assert!(validar_venta(Unidad::Kg, "Queso", 100_000).is_ok());
        assert!(validar_venta(Unidad::Kg, "Queso", 0).is_err());
        assert!(validar_venta(Unidad::Kg, "Queso", 100_001).is_err());
        assert!(validar_stock(Unidad::Kg, 1500, "stock").is_ok());
        assert!(validar_stock(Unidad::Unidad, 1500, "stock").is_err());
        assert!(validar_stock(Unidad::Kg, -1, "stock").is_err());
    }
}
