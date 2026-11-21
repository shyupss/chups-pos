// Texto de la boleta en 32 y 48 columnas comparado con los .txt de tests/boletas/.
// Para regenerarlos después de un cambio de diseño: ACTUALIZAR_BOLETAS=1 cargo test --test boleta_texto
// y revisar el diff antes de commitear.

use std::path::Path;

use pos_core::boleta::texto::{self, COLUMNAS_58MM, COLUMNAS_80MM};
use pos_core::boleta::{Boleta, Encabezado, LineaBoleta};
use pos_core::logo::LogoBn;
use pos_core::repositorios::productos::Unidad;
use pos_core::servicios::ventas::MedioPago;

fn negocio() -> Encabezado {
    Encabezado {
        nombre: "Almacén Don Pepe".into(),
        rut: Some("12.345.678-5".into()),
        direccion: Some("Av. Siempre Viva 742, Talca".into()),
        telefono: Some("+56 9 1234 5678".into()),
        mensaje_pie: "¡Gracias por su compra!".into(),
        logo: None,
    }
}

fn linea(nombre: &str, unidad: Unidad, cantidad_mil: i64, precio: i64) -> LineaBoleta {
    LineaBoleta {
        nombre: nombre.into(),
        unidad,
        cantidad_mil,
        precio_unitario: precio,
        subtotal: (precio * cantidad_mil + 500) / 1000,
    }
}

fn boleta(
    lineas: Vec<LineaBoleta>,
    medio: MedioPago,
    recibido: Option<i64>,
    redondeo: i64,
) -> Boleta {
    let subtotal: i64 = lineas.iter().map(|l| l.subtotal).sum();
    let total = subtotal + redondeo;
    Boleta {
        negocio: negocio(),
        folio: 123,
        fecha: "21-11-2026 14:05".into(),
        lineas,
        subtotal,
        descuento: 0,
        redondeo,
        total,
        medio_pago: medio,
        monto_recibido: recibido,
        vuelto: recibido.map(|r| r - total),
        cliente: None,
        leyenda: true,
        codigo_barras: true,
    }
}

fn comparar(nombre: &str, b: &Boleta) {
    for columnas in [COLUMNAS_58MM, COLUMNAS_80MM] {
        let obtenido = texto::a_texto(&texto::renderizar(b, columnas), columnas);
        for l in obtenido.lines() {
            assert!(
                l.chars().count() <= columnas,
                "{nombre}: la línea {l:?} pasa de {columnas} columnas"
            );
        }
        let ruta = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/boletas")
            .join(format!("{nombre}-{columnas}.txt"));
        if std::env::var_os("ACTUALIZAR_BOLETAS").is_some() {
            std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
            std::fs::write(&ruta, &obtenido).unwrap();
            continue;
        }
        let esperado = std::fs::read_to_string(&ruta)
            .unwrap_or_else(|_| panic!("falta {}: corre con ACTUALIZAR_BOLETAS=1", ruta.display()))
            .replace("\r\n", "\n");
        assert_eq!(
            obtenido,
            esperado,
            "{nombre} en {columnas} columnas no coincide con {}",
            ruta.display()
        );
    }
}

#[test]
fn venta_por_unidad_en_efectivo_con_vuelto() {
    let b = boleta(
        vec![
            linea("Coca-Cola 1,5 L", Unidad::Unidad, 2000, 2190),
            linea("Leche entera Colun 1 L", Unidad::Unidad, 3000, 1190),
            linea("Pan amasado", Unidad::Unidad, 6000, 250),
        ],
        MedioPago::Efectivo,
        Some(10_000),
        0,
    );
    assert_eq!(b.total, 9450);
    comparar("unidad-efectivo", &b);
}

#[test]
fn venta_por_kg_con_redondeo() {
    // 1.224 + 3.336 + 2.790 + 1.047 = 8.397: en efectivo se cobra 8.400 (+3).
    let b = boleta(
        vec![
            linea("Pan corriente (marraqueta)", Unidad::Kg, 532, 2300),
            linea("Queso mantecoso", Unidad::Kg, 318, 10_490),
            linea("Bebida 3 L", Unidad::Unidad, 1000, 2790),
            linea("Plátano", Unidad::Kg, 812, 1290),
        ],
        MedioPago::Efectivo,
        Some(20_000),
        3,
    );
    assert_eq!((b.subtotal, b.total, b.vuelto), (8397, 8400, Some(11_600)));
    comparar("kg-redondeo", &b);
}

#[test]
fn venta_con_debito() {
    let b = boleta(
        vec![linea("Detergente líquido 3 L", Unidad::Unidad, 1000, 6990)],
        MedioPago::Debito,
        None,
        0,
    );
    comparar("debito", &b);
}

#[test]
fn nombres_largos_y_logo() {
    let mut b = boleta(
        vec![
            linea(
                "Galletas de chocolate rellenas con crema de vainilla, paquete familiar de 6 unidades",
                Unidad::Unidad,
                2000,
                1890,
            ),
            linea("Jamón acaramelado laminado extra delgado", Unidad::Kg, 250, 11_990),
        ],
        MedioPago::Transferencia,
        None,
        0,
    );
    b.negocio.nombre = "Minimarket y Botillería La Esquina del Barrio".into();
    b.negocio.logo = Some(LogoBn {
        ancho: 16,
        alto: 2,
        puntos: vec![0xff; 4],
    });
    comparar("nombre-largo", &b);
}

#[test]
fn fiado_sin_leyenda_ni_codigo() {
    let mut b = boleta(
        vec![linea("Arroz grado 1, 1 kg", Unidad::Unidad, 2000, 1590)],
        MedioPago::Fiado,
        None,
        0,
    );
    b.cliente = Some("Rosa Martínez".into());
    b.leyenda = false;
    b.codigo_barras = false;
    b.descuento = 180;
    b.total = b.subtotal - b.descuento;
    comparar("fiado", &b);
}
