//! La boleta como líneas con alineación y estilo, para papel de 32 columnas (58 mm)
//! o 48 columnas (80 mm). La térmica ([`super::escpos`]) y el PDF las dibujan tal cual.

use super::formato::{detalle, folio, pesos, pesos_con_signo};
use super::Boleta;
use crate::servicios::ventas::MedioPago;

pub const COLUMNAS_58MM: usize = 32;
pub const COLUMNAS_80MM: usize = 48;
pub const LEYENDA: &str = "Comprobante interno sin validez tributaria";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Estilo {
    pub negrita: bool,
    pub doble_alto: bool,
    /// Cada carácter ocupa dos columnas: caben la mitad.
    pub doble_ancho: bool,
}

impl Estilo {
    pub const NORMAL: Estilo = Estilo {
        negrita: false,
        doble_alto: false,
        doble_ancho: false,
    };
    pub const NEGRITA: Estilo = Estilo {
        negrita: true,
        doble_alto: false,
        doble_ancho: false,
    };
    pub const TOTAL: Estilo = Estilo {
        negrita: true,
        doble_alto: true,
        doble_ancho: false,
    };
    pub const TITULO: Estilo = Estilo {
        negrita: true,
        doble_alto: true,
        doble_ancho: true,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alineacion {
    Izquierda,
    Centro,
    Derecha,
}

/// Una línea de la boleta. `Logo` y `CodigoBarras` marcan dónde va la imagen y las barras:
/// cada salida las dibuja a su manera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineaTexto {
    Texto {
        texto: String,
        alineacion: Alineacion,
        estilo: Estilo,
    },
    /// Texto a la izquierda y monto a la derecha. Ya se verificó que caben en una línea.
    Columnas {
        izquierda: String,
        derecha: String,
        estilo: Estilo,
    },
    Separador,
    Blanco,
    Logo,
    CodigoBarras(String),
}

fn largo(texto: &str) -> usize {
    texto.chars().count()
}

/// Corta en palabras para que ninguna línea pase de `ancho` caracteres
/// (una palabra más larga que el ancho se corta donde sea).
pub fn partir(texto: &str, ancho: usize) -> Vec<String> {
    let ancho = ancho.max(1);
    let mut lineas = Vec::new();
    let mut actual = String::new();
    for palabra in texto.split_whitespace() {
        let mut palabra: Vec<char> = palabra.chars().collect();
        while palabra.len() > ancho {
            if !actual.is_empty() {
                lineas.push(std::mem::take(&mut actual));
            }
            lineas.push(palabra.drain(..ancho).collect());
        }
        if palabra.is_empty() {
            continue;
        }
        let cabe = if actual.is_empty() {
            palabra.len() <= ancho
        } else {
            largo(&actual) + 1 + palabra.len() <= ancho
        };
        if !cabe {
            lineas.push(std::mem::take(&mut actual));
        }
        if !actual.is_empty() {
            actual.push(' ');
        }
        actual.extend(palabra);
    }
    if !actual.is_empty() {
        lineas.push(actual);
    }
    lineas
}

struct Armado {
    lineas: Vec<LineaTexto>,
    columnas: usize,
}

impl Armado {
    fn texto(&mut self, texto: &str, alineacion: Alineacion, estilo: Estilo) {
        let ancho = if estilo.doble_ancho {
            self.columnas / 2
        } else {
            self.columnas
        };
        for parte in partir(texto, ancho) {
            self.lineas.push(LineaTexto::Texto {
                texto: parte,
                alineacion,
                estilo,
            });
        }
    }

    /// Texto y monto en la misma línea; si no caben juntos, el texto arriba y el monto abajo.
    fn par(&mut self, izquierda: &str, derecha: &str, estilo: Estilo) {
        if largo(izquierda) + 1 + largo(derecha) <= self.columnas {
            self.lineas.push(LineaTexto::Columnas {
                izquierda: izquierda.to_owned(),
                derecha: derecha.to_owned(),
                estilo,
            });
        } else {
            self.texto(izquierda, Alineacion::Izquierda, estilo);
            self.texto(derecha, Alineacion::Derecha, estilo);
        }
    }

    fn push(&mut self, linea: LineaTexto) {
        self.lineas.push(linea);
    }
}

/// Arma las líneas de la boleta en el orden en que se imprimen.
pub fn renderizar(b: &Boleta, columnas: usize) -> Vec<LineaTexto> {
    use Alineacion::{Centro, Izquierda};
    let mut a = Armado {
        lineas: Vec::new(),
        columnas,
    };

    // 1. Negocio.
    if b.negocio.logo.is_some() {
        a.push(LineaTexto::Logo);
    }
    a.texto(&b.negocio.nombre, Centro, Estilo::TITULO);
    if let Some(rut) = &b.negocio.rut {
        a.texto(&format!("RUT {rut}"), Centro, Estilo::NORMAL);
    }
    if let Some(direccion) = &b.negocio.direccion {
        a.texto(direccion, Centro, Estilo::NORMAL);
    }
    if let Some(telefono) = &b.negocio.telefono {
        a.texto(&format!("Tel. {telefono}"), Centro, Estilo::NORMAL);
    }

    // 2. Folio y fecha.
    a.push(LineaTexto::Separador);
    a.texto(
        &format!("COMPROBANTE DE VENTA N° {}", folio(b.folio)),
        Centro,
        Estilo::NEGRITA,
    );
    a.texto(&b.fecha, Centro, Estilo::NORMAL);
    a.push(LineaTexto::Separador);

    // 3. Detalle.
    for l in &b.lineas {
        a.texto(&l.nombre, Izquierda, Estilo::NORMAL);
        a.par(
            &detalle(l.cantidad_mil, l.unidad, l.precio_unitario),
            &pesos(l.subtotal),
            Estilo::NORMAL,
        );
    }

    // 4. Totales.
    a.push(LineaTexto::Separador);
    a.par("Subtotal", &pesos(b.subtotal), Estilo::NORMAL);
    if b.descuento > 0 {
        a.par("Descuento", &pesos(-b.descuento), Estilo::NORMAL);
    }
    if b.redondeo != 0 {
        a.par("Redondeo", &pesos_con_signo(b.redondeo), Estilo::NORMAL);
    }
    a.par("TOTAL", &pesos(b.total), Estilo::TOTAL);

    // 5. Pago.
    a.push(LineaTexto::Separador);
    if b.medio_pago == MedioPago::Fiado {
        a.texto("FIADO", Centro, Estilo::TITULO);
        if let Some(cliente) = &b.cliente {
            a.texto(&format!("Cliente: {cliente}"), Izquierda, Estilo::NORMAL);
        }
    } else {
        a.par("Pago", b.medio_pago.nombre(), Estilo::NORMAL);
        if b.medio_pago == MedioPago::Efectivo {
            if let Some(recibido) = b.monto_recibido {
                a.par("Recibido", &pesos(recibido), Estilo::NORMAL);
            }
            if let Some(vuelto) = b.vuelto {
                a.par("Vuelto", &pesos(vuelto), Estilo::NEGRITA);
            }
        }
    }

    // 6. Pie.
    a.push(LineaTexto::Separador);
    a.texto(
        &format!("Productos: {}", b.cantidad_productos()),
        Izquierda,
        Estilo::NORMAL,
    );
    if !b.negocio.mensaje_pie.trim().is_empty() {
        a.push(LineaTexto::Blanco);
        a.texto(&b.negocio.mensaje_pie, Centro, Estilo::NORMAL);
    }
    if b.leyenda {
        a.push(LineaTexto::Blanco);
        a.texto(LEYENDA, Centro, Estilo::NORMAL);
    }
    if b.codigo_barras {
        a.push(LineaTexto::Blanco);
        a.push(LineaTexto::CodigoBarras(folio(b.folio)));
    }
    a.lineas
}

/// Línea de texto con el relleno de espacios ya puesto, tal como la recibe la impresora.
/// En doble ancho cada carácter ocupa dos columnas, así que se rellena sobre la mitad.
pub fn rellenar(texto: &str, alineacion: Alineacion, estilo: Estilo, columnas: usize) -> String {
    let ancho = if estilo.doble_ancho {
        columnas / 2
    } else {
        columnas
    };
    let sobra = ancho.saturating_sub(largo(texto));
    let izquierda = match alineacion {
        Alineacion::Izquierda => 0,
        Alineacion::Centro => sobra / 2,
        Alineacion::Derecha => sobra,
    };
    format!("{}{texto}", " ".repeat(izquierda))
}

pub fn columnas_texto(izquierda: &str, derecha: &str, columnas: usize) -> String {
    let espacios = columnas
        .saturating_sub(largo(izquierda) + largo(derecha))
        .max(1);
    format!("{izquierda}{}{derecha}", " ".repeat(espacios))
}

/// Vista de texto plano de la boleta, para pruebas y para revisar el diseño.
/// El doble ancho se muestra con un espacio después de cada letra.
pub fn a_texto(lineas: &[LineaTexto], columnas: usize) -> String {
    let mut salida = String::new();
    for linea in lineas {
        let texto = match linea {
            LineaTexto::Texto {
                texto,
                alineacion,
                estilo,
            } => {
                let relleno = rellenar(texto, *alineacion, *estilo, columnas);
                if estilo.doble_ancho {
                    let espacios = relleno.chars().take_while(|c| *c == ' ').count();
                    let letras: String = texto.chars().flat_map(|c| [c, ' ']).collect();
                    format!("{}{letras}", " ".repeat(espacios * 2))
                } else {
                    relleno
                }
            }
            LineaTexto::Columnas {
                izquierda, derecha, ..
            } => columnas_texto(izquierda, derecha, columnas),
            LineaTexto::Separador => "-".repeat(columnas),
            LineaTexto::Blanco => String::new(),
            LineaTexto::Logo => rellenar("[logo]", Alineacion::Centro, Estilo::NORMAL, columnas),
            LineaTexto::CodigoBarras(codigo) => rellenar(
                &format!("[código de barras {codigo}]"),
                Alineacion::Centro,
                Estilo::NORMAL,
                columnas,
            ),
        };
        salida.push_str(texto.trim_end());
        salida.push('\n');
    }
    salida
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parte_en_palabras() {
        assert_eq!(
            partir("Pan corriente (marraqueta)", 16),
            ["Pan corriente", "(marraqueta)"]
        );
        assert_eq!(
            partir("Supercalifragilístico", 8),
            ["Supercal", "ifragilí", "stico"]
        );
        assert_eq!(partir("  ", 10), Vec::<String>::new());
        assert_eq!(partir("Hola mundo", 10), ["Hola mundo"]);
    }

    #[test]
    fn rellena_segun_alineacion() {
        assert_eq!(
            rellenar("ab", Alineacion::Centro, Estilo::NORMAL, 6),
            "  ab"
        );
        assert_eq!(
            rellenar("ab", Alineacion::Derecha, Estilo::NORMAL, 6),
            "    ab"
        );
        assert_eq!(rellenar("ab", Alineacion::Centro, Estilo::TITULO, 8), " ab");
        assert_eq!(columnas_texto("Total", "$10", 12), "Total    $10");
    }
}
