//! Escribe un [`Libro`] como .xlsx: una hoja por tabla, con el título arriba, la fila de
//! encabezados congelada, montos y cantidades como números con formato y fechas en hora local.
//! Las hojas se escriben fila a fila (constant_memory): una exportación grande no se queda en
//! memoria.

use std::path::Path;

use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook, Worksheet, XlsxError};

use super::{Libro, Valor};
use crate::zona::{self, Zona};
use crate::{Error, Resultado};

fn error(e: XlsxError) -> Error {
    Error::Validacion(format!("No se pudo escribir el Excel: {e}"))
}

/// Los formatos de cada tipo de celda.
struct Formatos {
    titulo: Format,
    subtitulo: Format,
    encabezado: Format,
    pesos: Format,
    cantidad: Format,
    entero: Format,
    porcentaje: Format,
    fecha: Format,
    dia: Format,
}

impl Formatos {
    fn new() -> Formatos {
        Formatos {
            titulo: Format::new().set_bold().set_font_size(14),
            subtitulo: Format::new().set_font_color(Color::RGB(0x657080)),
            encabezado: Format::new()
                .set_bold()
                .set_background_color(Color::RGB(0xE1E5EA))
                .set_border_bottom(FormatBorder::Thin),
            pesos: Format::new().set_num_format("$#,##0"),
            cantidad: Format::new().set_num_format("#,##0.###"),
            entero: Format::new().set_num_format("#,##0"),
            porcentaje: Format::new().set_num_format("0.0%"),
            fecha: Format::new()
                .set_num_format("dd-mm-yyyy hh:mm")
                .set_align(FormatAlign::Left),
            dia: Format::new()
                .set_num_format("dd-mm-yyyy")
                .set_align(FormatAlign::Left),
        }
    }
}

/// Excel no acepta estos caracteres en el nombre de una hoja, ni más de 31.
fn nombre_hoja(nombre: &str, usados: &mut Vec<String>) -> String {
    let limpio: String = nombre
        .chars()
        .map(|c| if "[]:*?/\\".contains(c) { '-' } else { c })
        .take(28)
        .collect();
    let mut n = limpio.clone();
    let mut i = 2;
    while usados.iter().any(|u| u.eq_ignore_ascii_case(&n)) {
        n = format!("{limpio} {i}");
        i += 1;
    }
    usados.push(n.clone());
    n
}

fn escribir_celda(
    hoja: &mut Worksheet,
    fila: u32,
    col: u16,
    v: &Valor,
    f: &Formatos,
    zona: &dyn Zona,
) -> Result<(), XlsxError> {
    match v {
        Valor::Vacio => {}
        Valor::Texto(t) => {
            hoja.write_string(fila, col, t)?;
        }
        // Excel guarda los números como double: los pesos enteros (hasta 2^53) quedan exactos.
        Valor::Pesos(p) => {
            hoja.write_number_with_format(fila, col, *p as f64, &f.pesos)?;
        }
        Valor::Cantidad { mil, .. } => {
            hoja.write_number_with_format(fila, col, *mil as f64 / 1000.0, &f.cantidad)?;
        }
        Valor::Entero(n) => {
            hoja.write_number_with_format(fila, col, *n as f64, &f.entero)?;
        }
        Valor::Porcentaje(None) => {}
        Valor::Porcentaje(Some(pb)) => {
            hoja.write_number_with_format(fila, col, *pb as f64 / 10_000.0, &f.porcentaje)?;
        }
        Valor::Fecha(utc) => match zona::de_texto(utc) {
            Some(t) => {
                hoja.write_datetime_with_format(fila, col, zona.a_local(t), &f.fecha)?;
            }
            None => {
                hoja.write_string(fila, col, utc)?;
            }
        },
        Valor::Dia(d) => {
            hoja.write_datetime_with_format(fila, col, d, &f.dia)?;
        }
    }
    Ok(())
}

/// Escribe el libro en `ruta`. Es trabajo de disco y CPU: llamarlo fuera de los hilos async.
pub fn escribir(libro: &Libro, ruta: &Path, zona: &dyn Zona) -> Resultado<()> {
    let mut wb = Workbook::new();
    let f = Formatos::new();
    let mut usados = Vec::new();
    for t in &libro.tablas {
        let hoja = wb.add_worksheet_with_constant_memory().map_err(error)?;
        hoja.set_name(nombre_hoja(&t.nombre, &mut usados))
            .map_err(error)?;
        // Título, negocio y rango arriba; los encabezados en la fila 4 (congelada).
        hoja.write_string_with_format(0, 0, &libro.titulo, &f.titulo)
            .map_err(error)?;
        let detalle = [
            libro.negocio.as_str(),
            libro.rango.as_str(),
            &format!("Emitido el {}", libro.emitido),
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
        hoja.write_string_with_format(1, 0, &detalle, &f.subtitulo)
            .map_err(error)?;
        let primera = 3u32;
        for (c, col) in t.columnas.iter().enumerate() {
            let c = c as u16;
            hoja.set_column_width(c, f64::from(col.ancho.max(6)))
                .map_err(error)?;
            hoja.write_string_with_format(primera, c, &col.titulo, &f.encabezado)
                .map_err(error)?;
        }
        hoja.set_freeze_panes(primera + 1, 0).map_err(error)?;
        for (i, fila) in t.filas.iter().enumerate() {
            let r = primera + 1 + i as u32;
            for (c, v) in fila.iter().enumerate() {
                escribir_celda(hoja, r, c as u16, v, &f, zona).map_err(error)?;
            }
        }
        if !t.filas.is_empty() && !t.columnas.is_empty() {
            hoja.autofilter(
                primera,
                0,
                primera + t.filas.len() as u32,
                t.columnas.len() as u16 - 1,
            )
            .map_err(error)?;
        }
    }
    if libro.tablas.is_empty() {
        wb.add_worksheet();
    }
    wb.save(ruta).map_err(|e| match e {
        XlsxError::IoError(io) => Error::Validacion(format!(
            "No se pudo guardar el Excel (¿está abierto en otro programa?): {io}"
        )),
        otro => error(otro),
    })
}
