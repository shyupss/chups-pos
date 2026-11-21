//! Logo del negocio para la boleta: se escala, se pasa a blanco y negro y se guarda
//! como PNG de 1 bit (pesa pocos KB y la impresora térmica lo imprime tal cual).

use image::imageops::FilterType;
use image::{GrayImage, Luma};

use crate::error::validacion;
use crate::Resultado;

/// Ancho máximo en puntos: el área imprimible de una térmica de 58 mm es de 384 puntos.
pub const ANCHO_MAXIMO: u32 = 384;
pub const ALTO_MAXIMO: u32 = 160;
/// Archivos más grandes no se intentan abrir.
const BYTES_MAXIMOS: usize = 10 * 1024 * 1024;

/// Imagen de 1 bit lista para imprimir. Cada fila ocupa `bytes_por_fila` bytes,
/// el bit más alto es el punto de la izquierda y 1 es negro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogoBn {
    pub ancho: u32,
    pub alto: u32,
    pub puntos: Vec<u8>,
}

impl LogoBn {
    pub fn bytes_por_fila(&self) -> usize {
        self.ancho.div_ceil(8) as usize
    }

    pub fn es_negro(&self, x: u32, y: u32) -> bool {
        let byte = self.puntos[y as usize * self.bytes_por_fila() + (x / 8) as usize];
        byte & (0x80 >> (x % 8)) != 0
    }
}

/// Convierte un PNG o JPG cualquiera en el logo de la boleta (PNG de 1 bit).
pub fn procesar(archivo: &[u8]) -> Resultado<Vec<u8>> {
    if archivo.len() > BYTES_MAXIMOS {
        return validacion("La imagen pesa más de 10 MB: usa una más liviana");
    }
    let imagen = image::load_from_memory(archivo).map_err(|_| {
        crate::Error::Validacion("No se pudo leer la imagen: usa un PNG o un JPG".into())
    })?;
    let gris = sobre_blanco(&imagen.to_rgba8());
    let gris = escalar(gris);
    let logo = tramar(&gris);
    codificar_png(&logo)
}

/// Lee el logo guardado (o cualquier PNG/JPG) como puntos blanco y negro.
pub fn decodificar(png: &[u8]) -> Resultado<LogoBn> {
    let imagen = image::load_from_memory(png).map_err(|_| {
        crate::Error::Validacion("El logo guardado está dañado: vuelve a cargarlo".into())
    })?;
    let gris = sobre_blanco(&imagen.to_rgba8());
    Ok(umbral(&gris))
}

/// Las partes transparentes del PNG quedan blancas, como el papel.
fn sobre_blanco(rgba: &image::RgbaImage) -> GrayImage {
    GrayImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let luz = (u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114) / 1000;
        let a = u32::from(a);
        Luma([((luz * a + 255 * (255 - a)) / 255) as u8])
    })
}

/// Achica para que quepa en 384 × 160 sin deformar. Un logo chico no se agranda.
fn escalar(gris: GrayImage) -> GrayImage {
    let (ancho, alto) = gris.dimensions();
    if ancho <= ANCHO_MAXIMO && alto <= ALTO_MAXIMO {
        return gris;
    }
    // Proporción con enteros: se elige el lado que más limita.
    let (nuevo_ancho, nuevo_alto) =
        if u64::from(ancho) * u64::from(ALTO_MAXIMO) >= u64::from(alto) * u64::from(ANCHO_MAXIMO) {
            let a = (u64::from(alto) * u64::from(ANCHO_MAXIMO) / u64::from(ancho)).max(1);
            (ANCHO_MAXIMO, a as u32)
        } else {
            let a = (u64::from(ancho) * u64::from(ALTO_MAXIMO) / u64::from(alto)).max(1);
            (a as u32, ALTO_MAXIMO)
        };
    image::imageops::resize(&gris, nuevo_ancho, nuevo_alto, FilterType::Triangle)
}

/// Floyd–Steinberg con enteros: los grises se convierten en puntos más o menos juntos.
/// En un logo ya en blanco y negro el resultado es el mismo que con un umbral.
fn tramar(gris: &GrayImage) -> LogoBn {
    let (ancho, alto) = gris.dimensions();
    let (w, h) = (ancho as usize, alto as usize);
    let mut valores: Vec<i16> = gris.pixels().map(|p| i16::from(p.0[0])).collect();
    let mut logo = vacio(ancho, alto);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let viejo = valores[i].clamp(0, 255);
            let negro = viejo < 128;
            if negro {
                marcar(&mut logo, x as u32, y as u32);
            }
            let error = viejo - if negro { 0 } else { 255 };
            let mut repartir = |dx: isize, dy: usize, peso: i16| {
                let nx = x as isize + dx;
                if nx >= 0 && (nx as usize) < w && y + dy < h {
                    valores[(y + dy) * w + nx as usize] += error * peso / 16;
                }
            };
            repartir(1, 0, 7);
            repartir(-1, 1, 3);
            repartir(0, 1, 5);
            repartir(1, 1, 1);
        }
    }
    logo
}

fn umbral(gris: &GrayImage) -> LogoBn {
    let mut logo = vacio(gris.width(), gris.height());
    for (x, y, p) in gris.enumerate_pixels() {
        if p.0[0] < 128 {
            marcar(&mut logo, x, y);
        }
    }
    logo
}

fn vacio(ancho: u32, alto: u32) -> LogoBn {
    LogoBn {
        ancho,
        alto,
        puntos: vec![0; ancho.div_ceil(8) as usize * alto as usize],
    }
}

fn marcar(logo: &mut LogoBn, x: u32, y: u32) {
    let i = y as usize * logo.bytes_por_fila() + (x / 8) as usize;
    logo.puntos[i] |= 0x80 >> (x % 8);
}

/// PNG en escala de grises de 1 bit. En PNG el 1 es blanco, así que se invierte.
fn codificar_png(logo: &LogoBn) -> Resultado<Vec<u8>> {
    let mut salida = Vec::new();
    let mut codificador = png::Encoder::new(&mut salida, logo.ancho, logo.alto);
    codificador.set_color(png::ColorType::Grayscale);
    codificador.set_depth(png::BitDepth::One);
    let invertidos: Vec<u8> = logo.puntos.iter().map(|b| !b).collect();
    let error = |_| crate::Error::Validacion("No se pudo guardar el logo".into());
    let mut escritor = codificador.write_header().map_err(error)?;
    escritor.write_image_data(&invertidos).map_err(error)?;
    escritor.finish().map_err(error)?;
    Ok(salida)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_de_prueba(ancho: u32, alto: u32) -> Vec<u8> {
        // Mitad izquierda negra, mitad derecha transparente.
        let img = image::RgbaImage::from_fn(ancho, alto, |x, _| {
            if x < ancho / 2 {
                image::Rgba([0, 0, 0, 255])
            } else {
                image::Rgba([0, 0, 0, 0])
            }
        });
        let mut bytes = std::io::Cursor::new(Vec::new());
        img.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    #[test]
    fn escala_y_pasa_a_un_bit() {
        let png = procesar(&png_de_prueba(1000, 200)).unwrap();
        let logo = decodificar(&png).unwrap();
        assert_eq!((logo.ancho, logo.alto), (384, 76));
        assert!(logo.es_negro(10, 10));
        assert!(!logo.es_negro(380, 10), "lo transparente queda blanco");
        assert!(png.len() < 8 * 1024, "{} bytes", png.len());
    }

    #[test]
    fn un_logo_chico_no_se_agranda() {
        let logo = decodificar(&procesar(&png_de_prueba(100, 50)).unwrap()).unwrap();
        assert_eq!((logo.ancho, logo.alto), (100, 50));
        assert_eq!(logo.bytes_por_fila(), 13);
    }

    #[test]
    fn rechaza_lo_que_no_es_imagen() {
        assert!(procesar(b"no soy una imagen").is_err());
    }
}
