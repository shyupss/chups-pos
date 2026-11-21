//! Sacar y meter datos: tablas que se escriben en Excel o en PDF (reportes y exportaciones).
//!
//! Un [`Libro`] tiene un título, el negocio, el rango y una o más [`Tabla`]s. Cada celda lleva
//! su tipo ([`Valor`]): en Excel los montos y cantidades quedan como números con formato (se
//! pueden sumar) y las fechas como fechas en hora local; en el PDF, como texto en formato chileno.

pub mod excel;
pub mod pdf;
pub mod planilla;

use chrono::NaiveDate;

use crate::boleta::formato::{cantidad, pesos};
use crate::repositorios::productos::Unidad;
use crate::zona::{self, Zona};

/// Una celda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Valor {
    Vacio,
    Texto(String),
    /// Pesos enteros.
    Pesos(i64),
    /// Milésimas: unidades o kg.
    Cantidad {
        mil: i64,
        unidad: Unidad,
    },
    Entero(i64),
    /// Puntos básicos (1234 = 12,34 %); `None` si no aplica.
    Porcentaje(Option<i64>),
    /// Instante como lo guarda la base (UTC): se muestra en hora local.
    Fecha(String),
    /// Día local.
    Dia(NaiveDate),
}

impl Valor {
    pub fn texto(t: impl Into<String>) -> Valor {
        Valor::Texto(t.into())
    }

    pub fn opcional_texto(t: Option<impl Into<String>>) -> Valor {
        t.map_or(Valor::Vacio, |t| Valor::Texto(t.into()))
    }

    pub fn opcional_pesos(p: Option<i64>) -> Valor {
        p.map_or(Valor::Vacio, Valor::Pesos)
    }

    /// Se alinea a la derecha (números).
    pub fn es_numero(&self) -> bool {
        matches!(
            self,
            Valor::Pesos(_) | Valor::Cantidad { .. } | Valor::Entero(_) | Valor::Porcentaje(_)
        )
    }

    /// Como se lee en el PDF: "$12.345", "0,532 kg", "12,3 %", "21-11-2026 14:05".
    pub fn a_texto(&self, zona: &dyn Zona) -> String {
        match self {
            Valor::Vacio => String::new(),
            Valor::Texto(t) => t.clone(),
            Valor::Pesos(p) => pesos(*p),
            Valor::Cantidad { mil, unidad } => cantidad(*mil, *unidad),
            Valor::Entero(n) => cantidad(n * 1000, Unidad::Unidad),
            Valor::Porcentaje(None) => "—".into(),
            Valor::Porcentaje(Some(pb)) => {
                let signo = if *pb < 0 { "-" } else { "" };
                let abs = pb.unsigned_abs();
                // Un decimal, redondeado: 1234 pb = 12,3 %.
                let decimas = (abs + 5) / 10;
                format!("{signo}{},{} %", decimas / 10, decimas % 10)
            }
            Valor::Fecha(utc) => zona::de_texto(utc)
                .map(|t| zona.a_local(t).format("%d-%m-%Y %H:%M").to_string())
                .unwrap_or_else(|| utc.clone()),
            Valor::Dia(d) => d.format("%d-%m-%Y").to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Columna {
    pub titulo: String,
    /// Ancho aproximado en caracteres (Excel) y peso relativo en el PDF.
    pub ancho: u16,
}

impl Columna {
    pub fn new(titulo: &str, ancho: u16) -> Columna {
        Columna {
            titulo: titulo.into(),
            ancho,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tabla {
    /// Nombre de la hoja de Excel (hasta 31 caracteres) y subtítulo en el PDF.
    pub nombre: String,
    pub columnas: Vec<Columna>,
    pub filas: Vec<Vec<Valor>>,
}

impl Tabla {
    pub fn new(nombre: &str, columnas: Vec<Columna>) -> Tabla {
        Tabla {
            nombre: nombre.into(),
            columnas,
            filas: Vec::new(),
        }
    }

    pub fn fila(&mut self, fila: Vec<Valor>) {
        debug_assert_eq!(fila.len(), self.columnas.len());
        self.filas.push(fila);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Libro {
    /// "Reporte de ventas".
    pub titulo: String,
    pub negocio: String,
    /// "01-11-2026 al 21-11-2026" (vacío si no aplica).
    pub rango: String,
    /// Fecha y hora local de emisión.
    pub emitido: String,
    pub tablas: Vec<Tabla>,
}

/// "01-11-2026 al 21-11-2026", o un solo día.
pub fn texto_rango(desde: NaiveDate, hasta: NaiveDate) -> String {
    if desde == hasta {
        desde.format("%d-%m-%Y").to_string()
    } else {
        format!(
            "{} al {}",
            desde.format("%d-%m-%Y"),
            hasta.format("%d-%m-%Y")
        )
    }
}

/// Nombre de archivo sin caracteres que Windows no acepta: "ventas-2026-11-01-al-2026-11-21".
pub fn nombre_archivo(base: &str, desde: NaiveDate, hasta: NaiveDate) -> String {
    if desde == hasta {
        format!("{base}-{desde}")
    } else {
        format!("{base}-{desde}-al-{hasta}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zona::ZonaConCambio;

    #[test]
    fn celdas_en_formato_chileno() {
        let z = ZonaConCambio::chile_abril_2026();
        assert_eq!(Valor::Pesos(1234567).a_texto(&z), "$1.234.567");
        assert_eq!(
            Valor::Cantidad {
                mil: 532,
                unidad: Unidad::Kg
            }
            .a_texto(&z),
            "0,532 kg"
        );
        assert_eq!(Valor::Porcentaje(Some(1234)).a_texto(&z), "12,3 %");
        assert_eq!(Valor::Porcentaje(Some(-5)).a_texto(&z), "-0,1 %");
        assert_eq!(Valor::Porcentaje(None).a_texto(&z), "—");
        assert_eq!(
            Valor::Fecha("2026-04-05T03:30:00.000Z".into()).a_texto(&z),
            "04-04-2026 23:30"
        );
        assert_eq!(
            texto_rango(
                NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 11, 21).unwrap()
            ),
            "01-11-2026 al 21-11-2026"
        );
    }
}
