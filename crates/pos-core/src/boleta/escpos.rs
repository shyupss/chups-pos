//! Bytes ESC/POS para impresoras térmicas. Funciones puras: no tocan hardware.
//!
//! Comandos usados (todas las térmicas compatibles con Epson los entienden):
//! - `ESC @` inicializa · `ESC t n` tabla de caracteres · `ESC a n` alineación
//! - `ESC E n` negrita · `GS ! n` tamaño (doble alto / doble ancho)
//! - `GS v 0` imagen raster (logo) · `GS k` código de barras CODE128
//! - `ESC d n` avanza n líneas · `GS V 66 0` avanza hasta el cortador y corta
//! - `ESC p m t1 t2` pulso para abrir el cajón

use super::texto::{self, Alineacion, Estilo, LineaTexto};
use super::Boleta;
use crate::logo::LogoBn;
use crate::servicios::config::TablaCaracteres;

const ESC: u8 = 0x1b;
const GS: u8 = 0x1d;
const LF: u8 = 0x0a;

/// Inicializar la impresora: borra estilos y alineación que hayan quedado.
pub const INICIALIZAR: [u8; 2] = [ESC, b'@'];
/// Avanza hasta la posición del cortador y hace un corte parcial.
pub const CORTE: [u8; 4] = [GS, b'V', 66, 0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opciones {
    /// 32 (58 mm) o 48 (80 mm).
    pub columnas: usize,
    pub tabla: TablaCaracteres,
}

/// Número de la tabla para `ESC t n` (numeración de Epson, la que siguen las térmicas chinas).
fn numero_tabla(tabla: TablaCaracteres) -> u8 {
    match tabla {
        TablaCaracteres::Cp850 => 2,
        TablaCaracteres::Wpc1252 => 16,
        TablaCaracteres::Cp858 => 19,
    }
}

/// Byte de un carácter en CP850 (CP858 es igual, pero con € en 0xD5 en vez de ı).
fn cp850(c: char) -> Option<u8> {
    const ALTOS: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜø£Ø×ƒáíóúñÑªº¿®¬½¼¡«»";
    if let Some(i) = ALTOS.chars().position(|x| x == c) {
        return Some(0x80 + i as u8);
    }
    Some(match c {
        'Á' => 0xb5,
        'Â' => 0xb6,
        'À' => 0xb7,
        '©' => 0xb8,
        '¢' => 0xbd,
        '¥' => 0xbe,
        'ã' => 0xc6,
        'Ã' => 0xc7,
        '¤' => 0xcf,
        'ð' => 0xd0,
        'Ð' => 0xd1,
        'Ê' => 0xd2,
        'Ë' => 0xd3,
        'È' => 0xd4,
        'ı' => 0xd5,
        'Í' => 0xd6,
        'Î' => 0xd7,
        'Ï' => 0xd8,
        '¦' => 0xdd,
        'Ì' => 0xde,
        'Ó' => 0xe0,
        'ß' => 0xe1,
        'Ô' => 0xe2,
        'Ò' => 0xe3,
        'õ' => 0xe4,
        'Õ' => 0xe5,
        'µ' => 0xe6,
        'þ' => 0xe7,
        'Þ' => 0xe8,
        'Ú' => 0xe9,
        'Û' => 0xea,
        'Ù' => 0xeb,
        'ý' => 0xec,
        'Ý' => 0xed,
        '¯' => 0xee,
        '´' => 0xef,
        '±' => 0xf1,
        '¾' => 0xf3,
        '¶' => 0xf4,
        '§' => 0xf5,
        '÷' => 0xf6,
        '¸' => 0xf7,
        '°' => 0xf8,
        '¨' => 0xf9,
        '·' => 0xfa,
        '¹' => 0xfb,
        '³' => 0xfc,
        '²' => 0xfd,
        '\u{a0}' => 0xff,
        _ => return None,
    })
}

/// Byte de un carácter en Windows-1252: Latin-1 tal cual, más la tipografía de 0x80–0x9F.
fn wpc1252(c: char) -> Option<u8> {
    match c as u32 {
        0xa0..=0xff => Some(c as u32 as u8),
        _ => Some(match c {
            '€' => 0x80,
            '‚' => 0x82,
            '„' => 0x84,
            '…' => 0x85,
            '‘' => 0x91,
            '’' => 0x92,
            '“' => 0x93,
            '”' => 0x94,
            '•' => 0x95,
            '–' => 0x96,
            '—' => 0x97,
            '™' => 0x99,
            _ => return None,
        }),
    }
}

/// Reemplazo para lo que la tabla no tiene: la letra sin tilde, una versión ASCII o "?".
fn reemplazo(c: char) -> &'static str {
    const SIN_TILDE: &str = "ĀAāaĂAăaĄAąaĆCćcČCčcĎDďdĒEēeĘEęeĚEěeĞGğgĪIīiİIıiŁLłlŃNńnŇNňnŌOōoŐOőoŘRřrŚSśsŞSşsŠSšsŢTţtŤTťtŪUūuŮUůuŰUűuŸYŹZźzŻZżzŽZžz";
    let mut letras = SIN_TILDE.char_indices();
    while let Some((_, con)) = letras.next() {
        let (i, _) = letras.next().expect("pares");
        if con == c {
            return &SIN_TILDE[i..i + 1];
        }
    }
    match c {
        '‘' | '’' | '‚' | '´' => "'",
        '“' | '”' | '„' => "\"",
        '–' | '—' | '‐' | '−' => "-",
        '…' => "...",
        '•' => "*",
        '€' => "EUR",
        '™' => "TM",
        'Œ' => "OE",
        'œ' => "oe",
        '\u{a0}' | '\u{2007}' | '\u{202f}' => " ",
        _ => "?",
    }
}

/// Convierte texto UTF-8 a los bytes de la tabla de caracteres de la impresora.
/// Lo que la tabla no tiene sale sin tilde, en una versión ASCII o como "?".
pub fn codificar(texto: &str, tabla: TablaCaracteres) -> Vec<u8> {
    let mut salida = Vec::with_capacity(texto.len());
    for c in texto.chars() {
        if c.is_ascii() {
            // Los controles (tabulaciones, saltos) no deben llegar como comandos a la impresora.
            salida.push(if c.is_ascii_control() { b' ' } else { c as u8 });
            continue;
        }
        let byte = match tabla {
            TablaCaracteres::Cp858 if c == '€' => Some(0xd5),
            TablaCaracteres::Cp858 if c == 'ı' => None,
            TablaCaracteres::Cp850 | TablaCaracteres::Cp858 => cp850(c),
            TablaCaracteres::Wpc1252 => wpc1252(c),
        };
        match byte {
            Some(b) => salida.push(b),
            None => salida.extend(codificar(reemplazo(c), tabla)),
        }
    }
    salida
}

/// Acumula los comandos sin repetir los de estilo que ya están puestos.
struct Escritor {
    bytes: Vec<u8>,
    tabla: TablaCaracteres,
    estilo: Estilo,
}

impl Escritor {
    fn new(tabla: TablaCaracteres) -> Self {
        let mut bytes = Vec::with_capacity(2048);
        bytes.extend_from_slice(&INICIALIZAR);
        bytes.extend_from_slice(&[ESC, b't', numero_tabla(tabla)]);
        Escritor {
            bytes,
            tabla,
            estilo: Estilo::NORMAL,
        }
    }

    fn estilo(&mut self, estilo: Estilo) {
        if estilo.negrita != self.estilo.negrita {
            self.bytes
                .extend_from_slice(&[ESC, b'E', u8::from(estilo.negrita)]);
        }
        if (estilo.doble_alto, estilo.doble_ancho)
            != (self.estilo.doble_alto, self.estilo.doble_ancho)
        {
            let n = (if estilo.doble_ancho { 0x10 } else { 0 }) | u8::from(estilo.doble_alto);
            self.bytes.extend_from_slice(&[GS, b'!', n]);
        }
        self.estilo = estilo;
    }

    fn alineacion(&mut self, alineacion: Alineacion) {
        let n = match alineacion {
            Alineacion::Izquierda => 0,
            Alineacion::Centro => 1,
            Alineacion::Derecha => 2,
        };
        self.bytes.extend_from_slice(&[ESC, b'a', n]);
    }

    fn linea(&mut self, texto: &str) {
        self.bytes.extend(codificar(texto, self.tabla));
        self.bytes.push(LF);
    }

    fn logo(&mut self, logo: &LogoBn) {
        self.alineacion(Alineacion::Centro);
        let ancho = logo.bytes_por_fila() as u16;
        let alto = logo.alto as u16;
        self.bytes.extend_from_slice(&[GS, b'v', b'0', 0]);
        self.bytes.extend_from_slice(&ancho.to_le_bytes());
        self.bytes.extend_from_slice(&alto.to_le_bytes());
        self.bytes.extend_from_slice(&logo.puntos);
        self.alineacion(Alineacion::Izquierda);
    }

    /// CODE128 con el juego B ("{B"), números debajo de las barras.
    fn codigo_barras(&mut self, datos: &str) {
        let datos: Vec<u8> = datos.bytes().filter(|b| (0x20..0x7f).contains(b)).collect();
        self.alineacion(Alineacion::Centro);
        self.bytes.extend_from_slice(&[GS, b'h', 60]); // alto en puntos
        self.bytes.extend_from_slice(&[GS, b'w', 2]); // ancho del módulo
        self.bytes.extend_from_slice(&[GS, b'H', 2]); // números debajo
        self.bytes.extend_from_slice(&[GS, b'f', 0]);
        self.bytes
            .extend_from_slice(&[GS, b'k', 73, datos.len() as u8 + 2, b'{', b'B']);
        self.bytes.extend_from_slice(&datos);
        self.bytes.push(LF);
        self.alineacion(Alineacion::Izquierda);
    }

    fn cortar(&mut self) {
        self.estilo(Estilo::NORMAL);
        self.bytes.extend_from_slice(&[ESC, b'd', 3]);
        self.bytes.extend_from_slice(&CORTE);
    }
}

/// La boleta completa, lista para mandar a la impresora en modo RAW.
pub fn boleta(b: &Boleta, op: &Opciones) -> Vec<u8> {
    let mut e = Escritor::new(op.tabla);
    for linea in texto::renderizar(b, op.columnas) {
        match linea {
            LineaTexto::Texto {
                texto,
                alineacion,
                estilo,
            } => {
                e.estilo(estilo);
                e.linea(&texto::rellenar(&texto, alineacion, estilo, op.columnas));
            }
            LineaTexto::Columnas {
                izquierda,
                derecha,
                estilo,
            } => {
                e.estilo(estilo);
                e.linea(&texto::columnas_texto(&izquierda, &derecha, op.columnas));
            }
            LineaTexto::Separador => {
                e.estilo(Estilo::NORMAL);
                e.linea(&"-".repeat(op.columnas));
            }
            LineaTexto::Blanco => e.linea(""),
            LineaTexto::Logo => {
                if let Some(logo) = &b.negocio.logo {
                    e.logo(logo);
                }
            }
            LineaTexto::CodigoBarras(codigo) => e.codigo_barras(&codigo),
        }
    }
    e.cortar();
    e.bytes
}

/// Lo que se ve en la página de prueba, para mostrarlo también en pantalla.
pub const TEXTO_TILDES: &str = "ÁÉÍÓÚ ÑÜ ¿¡ áéíóú ñü";

/// Página de prueba: nombre, tildes, una regla del ancho del papel y un código de barras.
pub fn pagina_prueba(nombre_negocio: &str, op: &Opciones) -> Vec<u8> {
    let mut e = Escritor::new(op.tabla);
    let centro = |e: &mut Escritor, t: &str, estilo: Estilo| {
        e.estilo(estilo);
        for parte in texto::partir(
            t,
            if estilo.doble_ancho {
                op.columnas / 2
            } else {
                op.columnas
            },
        ) {
            e.linea(&texto::rellenar(
                &parte,
                Alineacion::Centro,
                estilo,
                op.columnas,
            ));
        }
    };
    centro(&mut e, "PÁGINA DE PRUEBA", Estilo::TITULO);
    let nombre = if nombre_negocio.trim().is_empty() {
        "(sin nombre de negocio)"
    } else {
        nombre_negocio
    };
    centro(&mut e, nombre, Estilo::NEGRITA);
    e.estilo(Estilo::NORMAL);
    e.linea("");
    centro(&mut e, TEXTO_TILDES, Estilo::NORMAL);
    centro(&mut e, TEXTO_TILDES, Estilo::TOTAL);
    e.estilo(Estilo::NORMAL);
    e.linea("");
    let regla: String = "1234567890".chars().cycle().take(op.columnas).collect();
    e.linea(&regla);
    e.linea(&"=".repeat(op.columnas));
    let tabla = match op.tabla {
        TablaCaracteres::Cp858 => "CP858",
        TablaCaracteres::Cp850 => "CP850",
        TablaCaracteres::Wpc1252 => "WPC1252",
    };
    centro(
        &mut e,
        &format!("Tabla {tabla} - {} columnas", op.columnas),
        Estilo::NORMAL,
    );
    e.linea("");
    e.codigo_barras("CHUPS-POS");
    e.cortar();
    e.bytes
}

/// Pulso para abrir el cajón: pin 2 (m = 0) o pin 5 (m = 1); la duración va en pasos de 2 ms.
pub fn pulso_cajon(pin: u8, duracion_ms: u16) -> Vec<u8> {
    let m = if pin == 5 { 1 } else { 0 };
    let encendido = (duracion_ms / 2).clamp(1, 255) as u8;
    vec![ESC, b'p', m, encendido, 250]
}

/// Varias copias en un solo trabajo de impresión: cada una lleva su corte.
pub fn copias(bytes: &[u8], cantidad: u8) -> Vec<u8> {
    bytes.repeat(usize::from(cantidad.max(1)))
}
