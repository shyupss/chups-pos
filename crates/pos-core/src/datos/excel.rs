//! Escribe .xlsx: una hoja por tabla, con el título arriba, la fila de encabezados congelada,
//! montos y cantidades como números con formato y fechas en hora local. Las hojas se escriben
//! fila a fila (constant_memory) con [`Escritor`]: una exportación grande no se queda en memoria.

use std::path::Path;

use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook, XlsxError};

use super::{Columna, Libro, Valor};
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

/// La hoja que se está escribiendo.
struct Hoja {
    indice: usize,
    encabezado: u32,
    columnas: u16,
    filas: u32,
}

/// Escribe un libro hoja por hoja y fila por fila. Es trabajo de disco y CPU: usarlo fuera de
/// los hilos async.
pub struct Escritor<'z> {
    wb: Workbook,
    f: Formatos,
    usados: Vec<String>,
    zona: &'z dyn Zona,
    /// Título, negocio, rango y emisión (vacío en las planillas de ejemplo: solo encabezados).
    titulo: String,
    detalle: String,
    hoja: Option<Hoja>,
    hojas: usize,
}

impl<'z> Escritor<'z> {
    pub fn new(
        titulo: &str,
        negocio: &str,
        rango: &str,
        emitido: &str,
        zona: &'z dyn Zona,
    ) -> Self {
        let emision = if emitido.is_empty() {
            String::new()
        } else {
            format!("Emitido el {emitido}")
        };
        Escritor {
            wb: Workbook::new(),
            f: Formatos::new(),
            usados: Vec::new(),
            zona,
            titulo: titulo.into(),
            detalle: [negocio, rango, emision.as_str()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · "),
            hoja: None,
            hojas: 0,
        }
    }

    /// El filtro de la hoja anterior, cuando ya se sabe cuántas filas tuvo.
    fn cerrar_hoja(&mut self) -> Resultado<()> {
        if let Some(h) = self.hoja.take() {
            if h.filas > 0 && h.columnas > 0 {
                self.wb
                    .worksheet_from_index(h.indice)
                    .map_err(error)?
                    .autofilter(h.encabezado, 0, h.encabezado + h.filas, h.columnas - 1)
                    .map_err(error)?;
            }
        }
        Ok(())
    }

    /// Empieza una hoja nueva con sus encabezados.
    pub fn hoja(&mut self, nombre: &str, columnas: &[Columna]) -> Resultado<()> {
        self.cerrar_hoja()?;
        let nombre = nombre_hoja(nombre, &mut self.usados);
        let hoja = self
            .wb
            .add_worksheet_with_constant_memory()
            .map_err(error)?;
        hoja.set_name(nombre).map_err(error)?;
        for (c, col) in columnas.iter().enumerate() {
            hoja.set_column_width(c as u16, f64::from(col.ancho.max(6)))
                .map_err(error)?;
        }
        let encabezado = if self.titulo.is_empty() {
            0
        } else {
            hoja.write_string_with_format(0, 0, &self.titulo, &self.f.titulo)
                .map_err(error)?;
            hoja.write_string_with_format(1, 0, &self.detalle, &self.f.subtitulo)
                .map_err(error)?;
            3
        };
        for (c, col) in columnas.iter().enumerate() {
            hoja.write_string_with_format(encabezado, c as u16, &col.titulo, &self.f.encabezado)
                .map_err(error)?;
        }
        hoja.set_freeze_panes(encabezado + 1, 0).map_err(error)?;
        self.hoja = Some(Hoja {
            indice: self.hojas,
            encabezado,
            columnas: columnas.len() as u16,
            filas: 0,
        });
        self.hojas += 1;
        Ok(())
    }

    /// Agrega una fila a la hoja actual.
    pub fn fila(&mut self, valores: &[Valor]) -> Resultado<()> {
        let Some(h) = self.hoja.as_mut() else {
            return Err(Error::Validacion("Falta empezar una hoja".into()));
        };
        h.filas += 1;
        let r = h.encabezado + h.filas;
        let hoja = self.wb.worksheet_from_index(h.indice).map_err(error)?;
        let f = &self.f;
        for (c, v) in valores.iter().enumerate() {
            let c = c as u16;
            match v {
                Valor::Vacio | Valor::Porcentaje(None) => Ok(&mut *hoja),
                Valor::Texto(t) => hoja.write_string(r, c, t),
                // Excel guarda los números como double: los pesos enteros (hasta 2^53) quedan exactos.
                Valor::Pesos(p) => hoja.write_number_with_format(r, c, *p as f64, &f.pesos),
                Valor::Cantidad { mil, .. } => {
                    hoja.write_number_with_format(r, c, *mil as f64 / 1000.0, &f.cantidad)
                }
                Valor::Entero(n) => hoja.write_number_with_format(r, c, *n as f64, &f.entero),
                Valor::Porcentaje(Some(pb)) => {
                    hoja.write_number_with_format(r, c, *pb as f64 / 10_000.0, &f.porcentaje)
                }
                Valor::Fecha(utc) => match zona::de_texto(utc) {
                    Some(t) => {
                        hoja.write_datetime_with_format(r, c, self.zona.a_local(t), &f.fecha)
                    }
                    None => hoja.write_string(r, c, utc),
                },
                Valor::Dia(d) => hoja.write_datetime_with_format(r, c, d, &f.dia),
            }
            .map_err(error)?;
        }
        Ok(())
    }

    pub fn guardar(mut self, ruta: &Path) -> Resultado<()> {
        self.cerrar_hoja()?;
        if self.hojas == 0 {
            self.wb.add_worksheet();
        }
        self.wb.save(ruta).map_err(|e| match e {
            XlsxError::IoError(io) => Error::Validacion(format!(
                "No se pudo guardar el Excel (¿está abierto en otro programa?): {io}"
            )),
            otro => error(otro),
        })
    }
}

/// Escribe un libro completo (los reportes, que son chicos) en `ruta`.
pub fn escribir(libro: &Libro, ruta: &Path, zona: &dyn Zona) -> Resultado<()> {
    let mut e = Escritor::new(
        &libro.titulo,
        &libro.negocio,
        &libro.rango,
        &libro.emitido,
        zona,
    );
    for t in &libro.tablas {
        e.hoja(&t.nombre, &t.columnas)?;
        for fila in &t.filas {
            e.fila(fila)?;
        }
    }
    e.guardar(ruta)
}
