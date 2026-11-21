// Texto del cierre de caja, la anulación y la nota de devolución en 32 y 48 columnas, comparado
// con los .txt de tests/boletas/. Para regenerarlos después de un cambio de diseño:
// ACTUALIZAR_BOLETAS=1 cargo test --test documentos, y revisar el diff antes de commitear.

use std::path::Path;

use pos_core::boleta::cierre::CierreCaja;
use pos_core::boleta::texto::{self, COLUMNAS_58MM, COLUMNAS_80MM};
use pos_core::boleta::{escpos, Documento, Encabezado};
use pos_core::servicios::config::TablaCaracteres;
use pos_core::servicios::turnos::{
    Conteo, CuadreEfectivo, MovimientoCaja, ResumenTurno, TipoMovimiento, TotalMedio, Turno,
};
use pos_core::servicios::ventas::MedioPago;

fn negocio() -> Encabezado {
    Encabezado {
        nombre: "Almacén Don Pepe".into(),
        rut: Some("12.345.678-5".into()),
        direccion: Some("Av. Siempre Viva 742, Talca".into()),
        telefono: None,
        mensaje_pie: "¡Gracias por su compra!".into(),
        logo: None,
    }
}

fn comparar(nombre: &str, d: &dyn Documento) {
    for columnas in [COLUMNAS_58MM, COLUMNAS_80MM] {
        let obtenido = texto::a_texto(&d.lineas(columnas), columnas);
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
    // Los bytes para la térmica salen de las mismas líneas y terminan con el corte.
    let bytes = escpos::documento(
        d,
        &escpos::Opciones {
            columnas: COLUMNAS_80MM,
            tabla: TablaCaracteres::Cp858,
        },
    );
    assert!(bytes.starts_with(&escpos::INICIALIZAR));
    assert!(bytes.ends_with(&escpos::CORTE));
}

fn medio(medio: MedioPago, cantidad: i64, monto: i64, redondeo: i64) -> TotalMedio {
    TotalMedio {
        medio,
        cantidad,
        monto,
        redondeo,
    }
}

fn turno(id: i64, inicial: i64, esperado: i64, contado: i64) -> Turno {
    Turno {
        id,
        abierto_en: "2026-11-21T12:12:00.000Z".into(),
        cerrado_en: Some("2026-11-22T00:05:00.000Z".into()),
        abierto: false,
        monto_inicial: inicial,
        efectivo_esperado: Some(esperado),
        efectivo_contado: Some(contado),
        diferencia: Some(contado - esperado),
        detalle_conteo: None,
        nota_cierre: None,
    }
}

#[test]
fn cierre_de_caja_completo_con_sobrante() {
    let mut t = turno(12, 20_000, 103_000, 103_100);
    t.detalle_conteo = Some(vec![
        Conteo {
            denominacion: 20_000,
            cantidad: 4,
        },
        Conteo {
            denominacion: 10_000,
            cantidad: 2,
        },
        Conteo {
            denominacion: 1_000,
            cantidad: 3,
        },
        Conteo {
            denominacion: 100,
            cantidad: 1,
        },
    ]);
    t.nota_cierre = Some("Sobran $100: un cliente no esperó el vuelto.".into());
    let c = CierreCaja {
        negocio: negocio(),
        apertura: "21-11-2026 09:12".into(),
        cierre: Some("21-11-2026 21:05".into()),
        resumen: ResumenTurno {
            turno: t,
            cantidad_ventas: 15,
            total_ventas: 123_450,
            ventas: vec![
                medio(MedioPago::Efectivo, 10, 80_000, -12),
                medio(MedioPago::Debito, 3, 30_000, 0),
                medio(MedioPago::Credito, 0, 0, 0),
                medio(MedioPago::Transferencia, 1, 3_450, 0),
                medio(MedioPago::Fiado, 1, 10_000, 0),
            ],
            redondeo_ventas: -12,
            abonos: vec![],
            anulaciones: vec![
                medio(MedioPago::Efectivo, 1, 2_000, 0),
                medio(MedioPago::Fiado, 1, 5_000, 0),
            ],
            devoluciones: vec![medio(MedioPago::Transferencia, 1, 1_000, 0)],
            redondeo_reembolsos: 0,
            movimientos: vec![
                MovimientoCaja {
                    id: 1,
                    tipo: TipoMovimiento::Ingreso,
                    monto: 10_000,
                    motivo: "Sencillo del banco".into(),
                    fecha: String::new(),
                },
                MovimientoCaja {
                    id: 2,
                    tipo: TipoMovimiento::Retiro,
                    monto: 5_000,
                    motivo: "Pago al panadero por la entrega de la mañana".into(),
                    fecha: String::new(),
                },
            ],
            efectivo: CuadreEfectivo {
                monto_inicial: 20_000,
                ventas: 80_000,
                abonos: 0,
                ingresos: 10_000,
                retiros: 5_000,
                reembolsos: 2_000,
                esperado: 103_000,
            },
        },
    };
    assert_eq!(c.titulo(), "Cierre de caja N° 000012");
    assert_eq!(c.nombre(), "cierre-000012");
    comparar("cierre-completo", &c);
}

#[test]
fn cierre_de_caja_simple_con_faltante() {
    let t = turno(3, 0, 4_220, 4_000);
    let c = CierreCaja {
        negocio: negocio(),
        apertura: "20-11-2026 22:40".into(),
        cierre: Some("21-11-2026 06:10".into()),
        resumen: ResumenTurno {
            turno: t,
            cantidad_ventas: 2,
            total_ventas: 4_220,
            ventas: vec![
                medio(MedioPago::Efectivo, 2, 4_220, -4),
                medio(MedioPago::Debito, 0, 0, 0),
                medio(MedioPago::Credito, 0, 0, 0),
                medio(MedioPago::Transferencia, 0, 0, 0),
                medio(MedioPago::Fiado, 0, 0, 0),
            ],
            redondeo_ventas: -4,
            abonos: vec![],
            anulaciones: vec![],
            devoluciones: vec![],
            redondeo_reembolsos: 0,
            movimientos: vec![],
            efectivo: CuadreEfectivo {
                monto_inicial: 0,
                ventas: 4_220,
                abonos: 0,
                ingresos: 0,
                retiros: 0,
                reembolsos: 0,
                esperado: 4_220,
            },
        },
    };
    comparar("cierre-faltante", &c);
}
