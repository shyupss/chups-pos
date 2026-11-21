use pos_core::boleta::{pdf, Boleta, Encabezado, LineaBoleta};
use pos_core::logo::LogoBn;
use pos_core::repositorios::productos::Unidad;
use pos_core::servicios::config::FormatoPdf;
use pos_core::servicios::ventas::MedioPago;

fn boleta(lineas: usize) -> Boleta {
    // Logo: un marco de 200 × 60 puntos.
    let (ancho, alto) = (200_u32, 60_u32);
    let por_fila = ancho.div_ceil(8) as usize;
    let mut puntos = vec![0u8; por_fila * alto as usize];
    for y in 0..alto {
        for x in 0..ancho {
            if x < 6 || y < 6 || x >= ancho - 6 || y >= alto - 6 || (x / 20 + y / 20) % 2 == 0 {
                puntos[y as usize * por_fila + (x / 8) as usize] |= 0x80 >> (x % 8);
            }
        }
    }
    let lineas: Vec<LineaBoleta> = (0..lineas)
        .map(|i| LineaBoleta {
            nombre: if i % 2 == 0 {
                format!("Pan amasado de campo con chicharrón Nº{i} — ñandú ¿sí? ¡ya! 😀")
            } else {
                "Queso mantecoso".into()
            },
            unidad: if i % 2 == 0 {
                Unidad::Unidad
            } else {
                Unidad::Kg
            },
            cantidad_mil: if i % 2 == 0 { 3000 } else { 318 },
            precio_unitario: if i % 2 == 0 { 250 } else { 10_490 },
            subtotal: if i % 2 == 0 { 750 } else { 3336 },
        })
        .collect();
    let subtotal = lineas.iter().map(|l| l.subtotal).sum::<i64>();
    let redondeo = -(subtotal % 10).min(5) + if subtotal % 10 > 5 { 10 } else { 0 };
    Boleta {
        negocio: Encabezado {
            nombre: "Almacén Don Pepe".into(),
            rut: Some("12.345.678-5".into()),
            direccion: Some("Av. Ñuble 1234, Ñuñoa".into()),
            telefono: Some("+56 9 1234 5678".into()),
            mensaje_pie: "¡Gracias por su compra! Vuelva pronto.".into(),
            logo: Some(LogoBn {
                ancho,
                alto,
                puntos,
            }),
        },
        folio: 123,
        fecha: "21-11-2026 14:05".into(),
        lineas,
        subtotal,
        descuento: 0,
        redondeo,
        total: subtotal + redondeo,
        medio_pago: MedioPago::Efectivo,
        monto_recibido: Some(50_000),
        vuelto: Some(50_000 - subtotal - redondeo),
        cliente: None,
        leyenda: true,
        codigo_barras: true,
        copia: false,
        anulada: None,
    }
}

fn paginas(pdf: &[u8]) -> usize {
    let veces = |buscado: &[u8]| pdf.windows(buscado.len()).filter(|w| *w == buscado).count();
    veces(b"/Type/Page") - veces(b"/Type/Pages")
}

#[test]
fn genera_pdf_ticket_y_carta() {
    for formato in [FormatoPdf::Ticket, FormatoPdf::Carta] {
        let bytes = pdf::boleta(&boleta(4), formato).unwrap();
        assert!(bytes.starts_with(b"%PDF"), "{formato:?}");
        assert!(bytes.len() > 1024, "{formato:?}: {} bytes", bytes.len());
        assert!(
            bytes.len() < 100 * 1024,
            "{formato:?}: {} bytes, las fuentes van comprimidas",
            bytes.len()
        );
        // Fuente incrustada y no la del sistema.
        assert!(bytes
            .windows(b"/FontFile2".len())
            .any(|w| w == b"/FontFile2"));
        if let Some(ruta) = std::env::var_os("GUARDAR_PDF_EJEMPLO") {
            let ruta = std::path::Path::new(&ruta).join(format!("ejemplo-{formato:?}.pdf"));
            std::fs::write(ruta, &bytes).unwrap();
        }
    }
    assert_eq!(pdf::nombre_archivo(123), "boleta-000123.pdf");
}

#[test]
fn una_boleta_larga_en_carta_usa_varias_paginas() {
    let corta = pdf::boleta(&boleta(4), FormatoPdf::Carta).unwrap();
    let larga = pdf::boleta(&boleta(90), FormatoPdf::Carta).unwrap();
    assert_eq!(paginas(&corta), 1);
    assert!(paginas(&larga) >= 2, "{} páginas", paginas(&larga));
    // El ticket siempre es una sola página, tan larga como haga falta.
    assert_eq!(
        paginas(&pdf::boleta(&boleta(90), FormatoPdf::Ticket).unwrap()),
        1
    );
}
