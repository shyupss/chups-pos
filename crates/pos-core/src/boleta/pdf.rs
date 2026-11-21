//! La boleta en PDF: ticket de 80 mm con el alto que necesite, o carta con la misma columna
//! centrada. Usa las mismas líneas que la térmica ([`texto::renderizar`]) y la fuente Inter
//! incrustada, así que no depende de las fuentes del sistema y las tildes salen bien.
//!
//! Las medidas (mm, puntos) son de diagramación, no montos: aquí sí se usan `f32`.

use barcoders::sym::code128::Code128;
use owned_ttf_parser::{AsFaceRef, OwnedFace};
use printpdf::{
    Color, ColorBits, ColorSpace, Greyscale, Image, ImageTransform, ImageXObject, IndirectFontRef,
    Mm, PdfDocument, PdfDocumentReference, PdfLayerReference, Px, Rect,
};

use super::escpos::reemplazo;
use super::texto::{Alineacion, Estilo, LineaTexto, COLUMNAS_80MM};
use super::{Boleta, Documento};
use crate::logo::LogoBn;
use crate::servicios::config::FormatoPdf;
use crate::{Error, Resultado};

pub(crate) const FUENTE_REGULAR: &[u8] = include_bytes!("../../fuentes/Inter-Regular-latin.ttf");
pub(crate) const FUENTE_NEGRITA: &[u8] = include_bytes!("../../fuentes/Inter-Bold-latin.ttf");

/// Ancho del papel del ticket y margen a cada lado.
const ANCHO_TICKET: f32 = 80.0;
const MARGEN: f32 = 4.0;
/// Carta: 215,9 × 279,4 mm, con la columna de 72 mm centrada.
pub(crate) const CARTA: (f32, f32) = (215.9, 279.4);
const MARGEN_CARTA: f32 = 15.0;
/// La térmica imprime a 203 dpi (8 puntos por mm): el logo sale del mismo tamaño que en papel.
const DPI_LOGO: f32 = 203.0;
pub(crate) const PT_A_MM: f32 = 25.4 / 72.0;
const MODULO_BARRAS: f32 = 0.33;
const ALTO_BARRAS: f32 = 10.0;

fn tamano(estilo: Estilo) -> f32 {
    match (estilo.doble_ancho, estilo.doble_alto) {
        (true, _) => 13.0,
        (false, true) => 11.0,
        _ => 8.0,
    }
}

pub(crate) fn error_pdf<E: std::fmt::Display>(e: E) -> Error {
    Error::Validacion(format!("No se pudo generar el PDF: {e}"))
}

/// Fuente para medir anchos y para saber qué caracteres tiene.
pub(crate) struct Medidas {
    regular: OwnedFace,
    negrita: OwnedFace,
}

impl Medidas {
    pub(crate) fn new() -> Resultado<Self> {
        Ok(Medidas {
            regular: OwnedFace::from_vec(FUENTE_REGULAR.to_vec(), 0).map_err(error_pdf)?,
            negrita: OwnedFace::from_vec(FUENTE_NEGRITA.to_vec(), 0).map_err(error_pdf)?,
        })
    }

    /// Ancho en mm de un texto en el tamaño dado.
    pub(crate) fn ancho(&self, texto: &str, negrita: bool, tamano_pt: f32) -> f32 {
        let cara = if negrita {
            &self.negrita
        } else {
            &self.regular
        }
        .as_face_ref();
        let unidades: u32 = texto
            .chars()
            .map(|c| {
                cara.glyph_index(c)
                    .or_else(|| cara.glyph_index('?'))
                    .and_then(|g| cara.glyph_hor_advance(g))
                    .map_or(0, u32::from)
            })
            .sum();
        unidades as f32 / f32::from(cara.units_per_em()) * tamano_pt * PT_A_MM
    }

    /// Reemplaza lo que la fuente recortada no tiene (sin tilde, ASCII o "?").
    pub(crate) fn seguro(&self, texto: &str) -> String {
        let cara = self.regular.as_face_ref();
        let mut salida = String::with_capacity(texto.len());
        for c in texto.chars() {
            if c == ' ' || cara.glyph_index(c).is_some() {
                salida.push(c);
            } else {
                salida.push_str(reemplazo(c));
            }
        }
        salida
    }
}

/// Alto en mm que ocupa cada línea.
fn alto(linea: &LineaTexto, logo: Option<&LogoBn>) -> f32 {
    match linea {
        LineaTexto::Texto { estilo, .. } | LineaTexto::Columnas { estilo, .. } => {
            tamano(*estilo) * PT_A_MM * 1.35
        }
        LineaTexto::Separador => 3.0,
        LineaTexto::Blanco => 2.5,
        LineaTexto::Logo => logo.map_or(0.0, |l| l.alto as f32 / DPI_LOGO * 25.4 + 2.0),
        LineaTexto::CodigoBarras(_) => ALTO_BARRAS + 6.0,
    }
}

/// Dibuja sobre una página. `x0` es el borde izquierdo de la columna y `ancho` su ancho.
struct Lienzo<'a> {
    doc: &'a PdfDocumentReference,
    capa: PdfLayerReference,
    regular: &'a IndirectFontRef,
    negrita: &'a IndirectFontRef,
    medidas: &'a Medidas,
    x0: f32,
    ancho: f32,
}

impl Lienzo<'_> {
    /// Escribe un texto con su línea base en `y`. Si no cabe, se achica la letra.
    fn texto(&self, texto: &str, x: f32, y: f32, estilo: Estilo) {
        let texto = self.medidas.seguro(texto);
        let mut tam = tamano(estilo);
        let w = self.medidas.ancho(&texto, estilo.negrita, tam);
        if w > self.ancho {
            tam = (tam * self.ancho / w).max(5.0);
        }
        let fuente = if estilo.negrita {
            self.negrita
        } else {
            self.regular
        };
        self.capa.use_text(texto, tam, Mm(x), Mm(y), fuente);
    }

    fn ancho_texto(&self, texto: &str, estilo: Estilo) -> f32 {
        self.medidas
            .ancho(&self.medidas.seguro(texto), estilo.negrita, tamano(estilo))
            .min(self.ancho)
    }

    fn linea(&self, linea: &LineaTexto, tope: f32, logo: Option<&LogoBn>) {
        let alto_linea = alto(linea, logo);
        let base = tope - alto_linea * 0.78;
        match linea {
            LineaTexto::Texto {
                texto,
                alineacion,
                estilo,
            } => {
                let w = self.ancho_texto(texto, *estilo);
                let x = match alineacion {
                    Alineacion::Izquierda => self.x0,
                    Alineacion::Centro => self.x0 + (self.ancho - w) / 2.0,
                    Alineacion::Derecha => self.x0 + self.ancho - w,
                };
                self.texto(texto, x, base, *estilo);
            }
            LineaTexto::Columnas {
                izquierda,
                derecha,
                estilo,
            } => {
                self.texto(izquierda, self.x0, base, *estilo);
                let w = self.ancho_texto(derecha, *estilo);
                self.texto(derecha, self.x0 + self.ancho - w, base, *estilo);
            }
            LineaTexto::Separador => {
                let y = tope - alto_linea / 2.0;
                self.capa.set_fill_color(gris(0.55));
                self.capa.add_rect(Rect::new(
                    Mm(self.x0),
                    Mm(y),
                    Mm(self.x0 + self.ancho),
                    Mm(y + 0.2),
                ));
                self.capa.set_fill_color(gris(0.0));
            }
            LineaTexto::Blanco => {}
            LineaTexto::Logo => {
                if let Some(logo) = logo {
                    let w = logo.ancho as f32 / DPI_LOGO * 25.4;
                    let h = logo.alto as f32 / DPI_LOGO * 25.4;
                    imagen(logo).add_to_layer(
                        self.capa.clone(),
                        ImageTransform {
                            translate_x: Some(Mm(self.x0 + (self.ancho - w) / 2.0)),
                            translate_y: Some(Mm(tope - h)),
                            dpi: Some(DPI_LOGO),
                            ..Default::default()
                        },
                    );
                }
            }
            LineaTexto::CodigoBarras(codigo) => self.barras(codigo, tope),
        }
    }

    /// CODE128 (juego B) dibujado con rectángulos, y el número debajo.
    fn barras(&self, codigo: &str, tope: f32) {
        let Ok(simbolo) = Code128::new(format!("Ɓ{codigo}")) else {
            return;
        };
        let modulos = simbolo.encode();
        let w = modulos.len() as f32 * MODULO_BARRAS;
        let x = self.x0 + (self.ancho - w) / 2.0;
        let abajo = tope - ALTO_BARRAS - 1.0;
        let mut i = 0;
        while i < modulos.len() {
            if modulos[i] == 1 {
                let inicio = i;
                while i < modulos.len() && modulos[i] == 1 {
                    i += 1;
                }
                let x1 = x + inicio as f32 * MODULO_BARRAS;
                let x2 = x + i as f32 * MODULO_BARRAS;
                self.capa.add_rect(Rect::new(
                    Mm(x1),
                    Mm(abajo),
                    Mm(x2),
                    Mm(abajo + ALTO_BARRAS),
                ));
            } else {
                i += 1;
            }
        }
        let numero = Estilo::NORMAL;
        let wn = self.ancho_texto(codigo, numero);
        self.texto(
            codigo,
            self.x0 + (self.ancho - wn) / 2.0,
            abajo - 3.5,
            numero,
        );
    }
}

pub(crate) fn gris(nivel: f32) -> Color {
    Color::Greyscale(Greyscale::new(nivel, None))
}

/// El logo de 1 bit como imagen en escala de grises (0 negro, 255 blanco).
fn imagen(logo: &LogoBn) -> Image {
    let mut datos = Vec::with_capacity((logo.ancho * logo.alto) as usize);
    for y in 0..logo.alto {
        for x in 0..logo.ancho {
            datos.push(if logo.es_negro(x, y) { 0 } else { 255 });
        }
    }
    Image::from(ImageXObject {
        width: Px(logo.ancho as usize),
        height: Px(logo.alto as usize),
        color_space: ColorSpace::Greyscale,
        bits_per_component: ColorBits::Bit8,
        interpolate: false,
        image_data: datos,
        image_filter: None,
        smask: None,
        clipping_bbox: None,
    })
}

/// Nombre del archivo: boleta-000123.pdf
pub fn nombre_archivo(folio: i64) -> String {
    format!("boleta-{}.pdf", super::formato::folio(folio))
}

/// Genera el PDF de la boleta.
pub fn boleta(b: &Boleta, formato: FormatoPdf) -> Resultado<Vec<u8>> {
    documento(b, formato)
}

/// Genera el PDF de cualquier comprobante con las líneas de 48 columnas.
pub fn documento(d: &dyn Documento, formato: FormatoPdf) -> Resultado<Vec<u8>> {
    let lineas = d.lineas(COLUMNAS_80MM);
    let logo = d.logo();
    let medidas = Medidas::new()?;
    let alto_contenido: f32 = lineas.iter().map(|l| alto(l, logo)).sum();

    let ancho_columna = ANCHO_TICKET - 2.0 * MARGEN;
    let (ancho_pagina, alto_pagina, x0, margen_vertical) = match formato {
        FormatoPdf::Ticket => (ANCHO_TICKET, alto_contenido + 2.0 * MARGEN, MARGEN, MARGEN),
        FormatoPdf::Carta => (
            CARTA.0,
            CARTA.1,
            (CARTA.0 - ancho_columna) / 2.0,
            MARGEN_CARTA,
        ),
    };
    let (doc, pagina, capa) =
        PdfDocument::new(d.titulo(), Mm(ancho_pagina), Mm(alto_pagina), "Boleta");
    let regular = doc.add_external_font(FUENTE_REGULAR).map_err(error_pdf)?;
    let negrita = doc.add_external_font(FUENTE_NEGRITA).map_err(error_pdf)?;

    let lienzo_en = |pagina, capa| Lienzo {
        doc: &doc,
        capa: doc.get_page(pagina).get_layer(capa),
        regular: &regular,
        negrita: &negrita,
        medidas: &medidas,
        x0,
        ancho: ancho_columna,
    };
    let mut lienzo = lienzo_en(pagina, capa);
    let mut tope = alto_pagina - margen_vertical;
    for linea in &lineas {
        let h = alto(linea, logo);
        // En carta, lo que no cabe pasa a la página siguiente.
        if tope - h < margen_vertical && formato == FormatoPdf::Carta {
            let (p, c) = lienzo
                .doc
                .add_page(Mm(ancho_pagina), Mm(alto_pagina), "Boleta");
            lienzo = lienzo_en(p, c);
            tope = alto_pagina - margen_vertical;
        }
        lienzo.linea(linea, tope, logo);
        tope -= h;
    }
    drop(lienzo);
    Ok(comprimir(doc.save_to_bytes().map_err(error_pdf)?))
}

/// printpdf no comprime las fuentes incrustadas (ni nada en debug): se comprimen todos los
/// streams con Flate. Deja el PDF en menos de la mitad. Si algo falla, queda el original.
pub(crate) fn comprimir(pdf: Vec<u8>) -> Vec<u8> {
    let Ok(mut doc) = lopdf::Document::load_mem(&pdf) else {
        return pdf;
    };
    doc.compress();
    let mut salida = Vec::with_capacity(pdf.len() / 2);
    match doc.save_to(&mut salida) {
        Ok(()) => salida,
        Err(_) => pdf,
    }
}
