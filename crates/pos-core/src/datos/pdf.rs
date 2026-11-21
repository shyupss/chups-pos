//! Un [`Libro`] en PDF tamaño carta: nombre del negocio, título, rango y fecha de emisión arriba,
//! y cada tabla con sus encabezados repetidos en cada página. Se pone apaisado si las columnas
//! no caben a lo alto. Usa la misma fuente Inter incrustada que la boleta.
//!
//! Las medidas (mm, puntos) son de diagramación, no montos: aquí sí se usan `f32`.

use printpdf::{IndirectFontRef, Mm, PdfDocument, PdfLayerReference, Rect};

use super::{Libro, Tabla};
use crate::boleta::pdf::{
    comprimir, error_pdf, gris, Medidas, CARTA, FUENTE_NEGRITA, FUENTE_REGULAR, PT_A_MM,
};
use crate::zona::Zona;
use crate::Resultado;

const MARGEN: f32 = 14.0;
const LETRA: f32 = 8.0;
const FILA: f32 = LETRA * PT_A_MM * 1.75;
const ESPACIO_COLUMNAS: f32 = 2.5;

struct Pagina<'a> {
    capa: PdfLayerReference,
    regular: &'a IndirectFontRef,
    negrita: &'a IndirectFontRef,
    medidas: &'a Medidas,
}

impl Pagina<'_> {
    /// Escribe `texto` desde `x` (o terminando en `x + ancho` si va a la derecha), cortado con
    /// "…" si no cabe en `ancho`.
    #[allow(clippy::too_many_arguments)]
    fn texto(
        &self,
        texto: &str,
        x: f32,
        y: f32,
        ancho: f32,
        tam: f32,
        negrita: bool,
        derecha: bool,
    ) {
        let mut t = self.medidas.seguro(texto);
        let medir = |s: &str| self.medidas.ancho(s, negrita, tam);
        if medir(&t) > ancho {
            while !t.is_empty() && medir(&format!("{t}...")) > ancho {
                t.pop();
            }
            t.push_str("...");
        }
        let x = if derecha { x + ancho - medir(&t) } else { x };
        let fuente = if negrita { self.negrita } else { self.regular };
        self.capa.use_text(t, tam, Mm(x), Mm(y), fuente);
    }

    fn raya(&self, x1: f32, x2: f32, y: f32, nivel: f32) {
        self.capa.set_fill_color(gris(nivel));
        self.capa
            .add_rect(Rect::new(Mm(x1), Mm(y), Mm(x2), Mm(y + 0.2)));
        self.capa.set_fill_color(gris(0.0));
    }
}

/// Anchos en mm de las columnas de una tabla, repartiendo el ancho útil según su peso.
fn anchos(t: &Tabla, util: f32) -> Vec<f32> {
    let total: f32 = t.columnas.iter().map(|c| f32::from(c.ancho.max(4))).sum();
    let libre = util - ESPACIO_COLUMNAS * t.columnas.len().saturating_sub(1) as f32;
    t.columnas
        .iter()
        .map(|c| libre * f32::from(c.ancho.max(4)) / total)
        .collect()
}

/// Dibuja el libro. Es trabajo de CPU: llamarlo fuera de los hilos async.
pub fn generar(libro: &Libro, zona: &dyn Zona) -> Resultado<Vec<u8>> {
    // Apaisado si alguna tabla tiene muchas columnas.
    let pesos_max = libro
        .tablas
        .iter()
        .map(|t| t.columnas.iter().map(|c| u32::from(c.ancho)).sum::<u32>())
        .max()
        .unwrap_or(0);
    let (ancho_pag, alto_pag) = if pesos_max > 110 {
        (CARTA.1, CARTA.0)
    } else {
        CARTA
    };
    let util = ancho_pag - 2.0 * MARGEN;
    let medidas = Medidas::new()?;
    let (doc, p0, c0) = PdfDocument::new(&libro.titulo, Mm(ancho_pag), Mm(alto_pag), "Reporte");
    let regular = doc.add_external_font(FUENTE_REGULAR).map_err(error_pdf)?;
    let negrita = doc.add_external_font(FUENTE_NEGRITA).map_err(error_pdf)?;
    let pagina = |p, c| Pagina {
        capa: doc.get_page(p).get_layer(c),
        regular: &regular,
        negrita: &negrita,
        medidas: &medidas,
    };
    let mut numero = 1;
    let pie = |pg: &Pagina, n: i32| {
        pg.texto(
            &format!("{} · Página {n}", libro.titulo),
            MARGEN,
            MARGEN / 2.0,
            util,
            7.0,
            false,
            true,
        );
    };

    let mut pg = pagina(p0, c0);
    let mut y = alto_pag - MARGEN;
    // Encabezado del documento.
    pg.texto(&libro.negocio, MARGEN, y - 5.0, util, 14.0, true, false);
    y -= 11.0;
    pg.texto(&libro.titulo, MARGEN, y, util, 12.0, true, false);
    y -= 5.5;
    let detalle = [
        libro.rango.as_str(),
        &format!("Emitido el {}", libro.emitido),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" · ");
    pg.texto(&detalle, MARGEN, y, util, 9.0, false, false);
    y -= 4.0;
    pg.raya(MARGEN, ancho_pag - MARGEN, y, 0.55);
    y -= 6.0;

    for t in &libro.tablas {
        let w = anchos(t, util);
        let derecha: Vec<bool> = (0..t.columnas.len())
            .map(|c| t.filas.iter().any(|f| f[c].es_numero()))
            .collect();
        let encabezados = |pg: &Pagina, y: f32| {
            let mut x = MARGEN;
            for (c, col) in t.columnas.iter().enumerate() {
                pg.texto(&col.titulo, x, y, w[c], LETRA, true, derecha[c]);
                x += w[c] + ESPACIO_COLUMNAS;
            }
            pg.raya(MARGEN, ancho_pag - MARGEN, y - 1.6, 0.35);
        };
        // El título de la tabla no queda solo al pie de una página.
        if y - 4.0 * FILA < MARGEN {
            pie(&pg, numero);
            let (p, c) = doc.add_page(Mm(ancho_pag), Mm(alto_pag), "Reporte");
            pg = pagina(p, c);
            numero += 1;
            y = alto_pag - MARGEN - 4.0;
        }
        pg.texto(&t.nombre, MARGEN, y, util, 10.0, true, false);
        y -= FILA * 1.3;
        encabezados(&pg, y);
        y -= FILA;
        if t.filas.is_empty() {
            pg.texto("Sin datos", MARGEN, y, util, LETRA, false, false);
            y -= FILA;
        }
        for fila in &t.filas {
            if y < MARGEN + FILA {
                pie(&pg, numero);
                let (p, c) = doc.add_page(Mm(ancho_pag), Mm(alto_pag), "Reporte");
                pg = pagina(p, c);
                numero += 1;
                y = alto_pag - MARGEN - 4.0;
                encabezados(&pg, y);
                y -= FILA;
            }
            let mut x = MARGEN;
            for (c, v) in fila.iter().enumerate() {
                pg.texto(&v.a_texto(zona), x, y, w[c], LETRA, false, derecha[c]);
                x += w[c] + ESPACIO_COLUMNAS;
            }
            y -= FILA;
        }
        y -= FILA;
    }
    pie(&pg, numero);
    drop(pg);
    Ok(comprimir(doc.save_to_bytes().map_err(error_pdf)?))
}
