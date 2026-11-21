use pos_core::boleta::escpos::{self, Opciones, CORTE, INICIALIZAR};
use pos_core::boleta::{Boleta, Encabezado, LineaBoleta};
use pos_core::logo::LogoBn;
use pos_core::repositorios::productos::Unidad;
use pos_core::servicios::config::TablaCaracteres::{Cp850, Cp858, Wpc1252};
use pos_core::servicios::ventas::MedioPago;

fn contiene(bytes: &[u8], buscado: &[u8]) -> bool {
    bytes.windows(buscado.len()).any(|w| w == buscado)
}

fn veces(bytes: &[u8], buscado: &[u8]) -> usize {
    bytes
        .windows(buscado.len())
        .filter(|w| *w == buscado)
        .count()
}

#[test]
fn convierte_tildes_enie_y_signos_a_cp858_y_cp850() {
    let texto = "áéíóú ÁÉÍÓÚ ñÑ üÜ ¿¡ °";
    let esperado: Vec<u8> = vec![
        0xa0, 0x82, 0xa1, 0xa2, 0xa3, b' ', // á é í ó ú
        0xb5, 0x90, 0xd6, 0xe0, 0xe9, b' ', // Á É Í Ó Ú
        0xa4, 0xa5, b' ', // ñ Ñ
        0x81, 0x9a, b' ', // ü Ü
        0xa8, 0xad, b' ', // ¿ ¡
        0xf8, // °
    ];
    assert_eq!(escpos::codificar(texto, Cp858), esperado);
    assert_eq!(escpos::codificar(texto, Cp850), esperado);

    // El € solo existe en CP858 (0xD5, donde CP850 tiene la ı).
    assert_eq!(escpos::codificar("5€", Cp858), [b'5', 0xd5]);
    assert_eq!(escpos::codificar("5€", Cp850), b"5EUR");
    assert_eq!(escpos::codificar("ı", Cp850), [0xd5]);
    assert_eq!(escpos::codificar("ı", Cp858), b"i");
}

#[test]
fn reemplaza_lo_que_la_tabla_no_tiene() {
    // Emoji: "?". Letras de otros idiomas: sin tilde. Tipografía: su versión ASCII.
    assert_eq!(escpos::codificar("Pan 😀 ok", Cp858), b"Pan ? ok");
    assert_eq!(escpos::codificar("Łódź", Cp858), [b'L', 0xa2, b'd', b'z']);
    assert_eq!(
        escpos::codificar("“Oferta” – 2…", Cp850),
        b"\"Oferta\" - 2..."
    );
    // Los caracteres de control no pasan: podrían ser comandos.
    assert_eq!(escpos::codificar("a\tb\u{1b}c", Cp858), b"a b c");
}

#[test]
fn windows_1252_usa_latin_1_y_su_tipografia() {
    assert_eq!(
        escpos::codificar("ñÑáÉ¿¡°€“”", Wpc1252),
        [0xf1, 0xd1, 0xe1, 0xc9, 0xbf, 0xa1, 0xb0, 0x80, 0x93, 0x94]
    );
    assert_eq!(escpos::codificar("😀", Wpc1252), b"?");
}

fn boleta(con_logo: bool) -> Boleta {
    Boleta {
        negocio: Encabezado {
            nombre: "Almacén Ñuñoa".into(),
            rut: None,
            direccion: None,
            telefono: None,
            mensaje_pie: "Gracias".into(),
            logo: con_logo.then(|| LogoBn {
                ancho: 20,
                alto: 3,
                puntos: vec![0b1010_1010; 9],
            }),
        },
        folio: 42,
        fecha: "21-11-2026 14:05".into(),
        lineas: vec![LineaBoleta {
            nombre: "Pan".into(),
            unidad: Unidad::Kg,
            cantidad_mil: 532,
            precio_unitario: 2300,
            subtotal: 1224,
        }],
        subtotal: 1224,
        descuento: 0,
        redondeo: -4,
        total: 1220,
        medio_pago: MedioPago::Efectivo,
        monto_recibido: Some(2000),
        vuelto: Some(780),
        cliente: None,
        leyenda: true,
        codigo_barras: true,
    }
}

#[test]
fn la_boleta_inicializa_corta_y_lleva_codigo_y_doble_alto() {
    let op = Opciones {
        columnas: 32,
        tabla: Cp858,
    };
    let bytes = escpos::boleta(&boleta(false), &op);

    assert!(bytes.starts_with(&INICIALIZAR), "empieza con ESC @");
    assert_eq!(&bytes[2..5], [0x1b, b't', 19], "elige CP858 (ESC t 19)");
    assert!(bytes.ends_with(&CORTE), "termina con el corte GS V 66 0");
    assert!(
        contiene(&bytes, &[0x1d, b'!', 0x01]),
        "TOTAL en doble alto (GS ! 1)"
    );
    assert!(
        contiene(&bytes, &[0x1d, b'!', 0x11]),
        "nombre en doble alto y ancho"
    );
    assert!(contiene(&bytes, &[0x1b, b'E', 1]), "negritas");
    // CODE128 con el folio: GS k 73 n {B 000042
    assert!(contiene(
        &bytes,
        &[0x1d, b'k', 73, 8, b'{', b'B', b'0', b'0', b'0', b'0', b'4', b'2']
    ));
    // El texto va en la tabla elegida: "Almacén" con la é en 0x82.
    assert!(contiene(
        &bytes,
        &[b'A', b'l', b'm', b'a', b'c', 0x82, b'n']
    ));
    assert!(contiene(&bytes, b"TOTAL"));
    assert!(contiene(&bytes, b"$1.220"));
    assert!(
        !contiene(&bytes, &[0x1d, b'v', b'0']),
        "sin logo no hay imagen"
    );

    let otra = escpos::boleta(
        &boleta(false),
        &Opciones {
            columnas: 32,
            tabla: Wpc1252,
        },
    );
    assert_eq!(&otra[2..5], [0x1b, b't', 16], "WPC1252 es ESC t 16");
}

#[test]
fn el_logo_va_como_imagen_raster() {
    let bytes = escpos::boleta(
        &boleta(true),
        &Opciones {
            columnas: 48,
            tabla: Cp850,
        },
    );
    // GS v 0, modo 0, 3 bytes por fila (20 puntos), 3 filas, y los 9 bytes de puntos.
    let mut esperado = vec![0x1d, b'v', b'0', 0, 3, 0, 3, 0];
    esperado.extend([0b1010_1010; 9]);
    assert!(contiene(&bytes, &esperado));
}

#[test]
fn copias_en_un_solo_trabajo() {
    let una = escpos::boleta(
        &boleta(false),
        &Opciones {
            columnas: 32,
            tabla: Cp858,
        },
    );
    let dos = escpos::copias(&una, 2);
    assert_eq!(dos.len(), una.len() * 2);
    assert_eq!(veces(&dos, &CORTE), 2);
}

#[test]
fn pulso_del_cajon_para_pin_2_y_5() {
    assert_eq!(escpos::pulso_cajon(2, 100), [0x1b, b'p', 0, 50, 250]);
    assert_eq!(escpos::pulso_cajon(5, 100), [0x1b, b'p', 1, 50, 250]);
    assert_eq!(escpos::pulso_cajon(2, 1000)[3], 255, "máximo 510 ms");
}

#[test]
fn pagina_de_prueba_con_tildes_regla_y_codigo() {
    let bytes = escpos::pagina_prueba(
        "Don Pepe",
        &Opciones {
            columnas: 48,
            tabla: Cp858,
        },
    );
    assert!(bytes.starts_with(&INICIALIZAR) && bytes.ends_with(&CORTE));
    assert!(contiene(
        &bytes,
        &escpos::codificar(escpos::TEXTO_TILDES, Cp858)
    ));
    let regla: Vec<u8> = b"1234567890".iter().copied().cycle().take(48).collect();
    assert!(contiene(&bytes, &regla), "regla de 48 caracteres");
    assert!(contiene(&bytes, b"{BCHUPS-POS"));
    assert!(contiene(&bytes, b"Don Pepe"));
}
