// Turnos de caja: abrir, vender, ingresos y retiros, efectivo esperado y cierre con conteo.

mod comun;

use pos_core::servicios::espera::{self, NuevaEnEspera};
use pos_core::servicios::seguridad::{self, ControlPin};
use pos_core::servicios::turnos::{self, Conteo, TipoMovimiento};
use pos_core::servicios::ventas::{self, ItemPedido, MedioPago, NuevaVenta, VentaConfirmada};
use pos_core::{db, Error, FalloPin};
use sqlx::SqlitePool;

fn pedido(producto_id: i64, cantidad_mil: i64, medio: MedioPago) -> NuevaVenta {
    NuevaVenta {
        items: vec![ItemPedido {
            producto_id,
            cantidad_mil,
        }],
        medio_pago: medio,
        monto_recibido: None,
        descuento: 0,
        cliente_id: None,
    }
}

async fn vender(
    pool: &SqlitePool,
    producto_id: i64,
    cantidad_mil: i64,
    medio: MedioPago,
) -> VentaConfirmada {
    ventas::confirmar(pool, &pedido(producto_id, cantidad_mil, medio))
        .await
        .unwrap()
}

async fn movimiento(
    pool: &SqlitePool,
    tipo: TipoMovimiento,
    monto: i64,
    motivo: &str,
) -> Result<(), Error> {
    turnos::registrar_movimiento(pool, &ControlPin::new(), tipo, monto, motivo, None)
        .await
        .map(|_| ())
}

#[tokio::test]
async fn sin_caja_abierta_no_se_vende_pero_se_puede_dejar_en_espera() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let pan = comun::crear(&pool, "P", "Pan", 250, 10).await;

    let estado = turnos::estado_caja(&pool).await.unwrap();
    assert!(estado.turno.is_none());
    assert_eq!(estado.ultimo_monto_inicial, None);

    let r = ventas::confirmar(&pool, &pedido(pan.id, 2000, MedioPago::Efectivo)).await;
    assert!(
        matches!(&r, Err(Error::Validacion(m)) if m == "Abre la caja para vender"),
        "{r:?}"
    );
    // Nada quedó a medias: ni venta ni stock descontado.
    let ventas: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ventas")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ventas, 0);
    let p = pos_core::servicios::productos::buscar_por_codigo(&pool, "P")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(p.stock_mil, 10_000);

    // Las ventas en espera no necesitan caja.
    let id = espera::poner(
        &pool,
        &NuevaEnEspera {
            nombre: None,
            items: vec![ItemPedido {
                producto_id: pan.id,
                cantidad_mil: 2000,
            }],
        },
    )
    .await
    .unwrap();
    let r = espera::recuperar(&pool, id, None).await.unwrap();
    assert_eq!(r.lineas.len(), 1);

    // Con la caja abierta, sí.
    turnos::abrir(&pool, 10_000).await.unwrap();
    let v = vender(&pool, pan.id, 2000, MedioPago::Efectivo).await;
    let turno_id: Option<i64> = sqlx::query_scalar("SELECT turno_id FROM ventas WHERE id = ?")
        .bind(v.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        turno_id,
        Some(turnos::estado_caja(&pool).await.unwrap().turno.unwrap().id)
    );
}

#[tokio::test]
async fn no_se_pueden_abrir_dos_cajas() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let t = turnos::abrir(&pool, 20_000).await.unwrap();
    assert!(t.abierto && t.cerrado_en.is_none());
    assert_eq!(t.monto_inicial, 20_000);
    assert!(matches!(
        turnos::abrir(&pool, 0).await,
        Err(Error::Conflicto(m)) if m == "La caja ya está abierta"
    ));
    assert!(matches!(
        turnos::abrir(&pool, -1).await,
        Err(Error::Validacion(_))
    ));

    // Se cierra y se puede abrir otro; se sugiere el último monto inicial.
    turnos::cerrar(&pool, &ControlPin::new(), 20_000, None, None, None)
        .await
        .unwrap();
    let estado = turnos::estado_caja(&pool).await.unwrap();
    assert!(estado.turno.is_none());
    assert_eq!(estado.ultimo_monto_inicial, Some(20_000));
    let otro = turnos::abrir(&pool, 15_000).await.unwrap();
    assert_ne!(otro.id, t.id);
    turnos::cerrar(&pool, &ControlPin::new(), 15_000, None, None, None)
        .await
        .unwrap();
    assert!(matches!(
        turnos::cerrar(&pool, &ControlPin::new(), 0, None, None, None).await,
        Err(Error::Validacion(m)) if m == "La caja ya está cerrada"
    ));
}

#[tokio::test]
async fn el_turno_sigue_abierto_al_cerrar_y_abrir_la_app() {
    let dir = tempfile::tempdir().unwrap();
    let ruta = dir.path().join("caja.db");
    let pool = db::abrir(&ruta).await.unwrap();
    let t = turnos::abrir(&pool, 5000).await.unwrap();
    pool.close().await;

    let pool = db::abrir(&ruta).await.unwrap();
    let estado = turnos::estado_caja(&pool).await.unwrap();
    assert_eq!(estado.turno.map(|t| t.id), Some(t.id));
}

#[tokio::test]
async fn calcula_el_efectivo_esperado() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let pan = comun::crear_kg(&pool, "P", "Pan", 2300, 100_000).await;
    let bebida = comun::crear(&pool, "B", "Bebida", 1500, 100).await;
    let t = turnos::abrir(&pool, 20_000).await.unwrap();

    // Efectivo: 0,532 kg de pan = 1.224 -> se cobra 1.220 (redondeo -4).
    let a = vender(&pool, pan.id, 532, MedioPago::Efectivo).await;
    assert_eq!((a.total, a.redondeo), (1220, -4));
    // Efectivo: 2 bebidas = 3.000 (sin redondeo). Débito: 0,532 kg = 1.224 (débito no redondea).
    vender(&pool, bebida.id, 2000, MedioPago::Efectivo).await;
    vender(&pool, pan.id, 532, MedioPago::Debito).await;
    vender(&pool, bebida.id, 1000, MedioPago::Transferencia).await;

    movimiento(&pool, TipoMovimiento::Ingreso, 10_000, "Sencillo del banco")
        .await
        .unwrap();
    movimiento(&pool, TipoMovimiento::Retiro, 5_000, "Pago al panadero")
        .await
        .unwrap();

    let r = turnos::resumen(&pool, t.id).await.unwrap();
    assert_eq!(r.cantidad_ventas, 4);
    assert_eq!(r.total_ventas, 1220 + 3000 + 1224 + 1500);
    assert_eq!(r.redondeo_ventas, -4);
    let por_medio: Vec<(MedioPago, i64, i64)> = r
        .ventas
        .iter()
        .map(|t| (t.medio, t.cantidad, t.monto))
        .collect();
    assert_eq!(
        por_medio,
        [
            (MedioPago::Efectivo, 2, 4220),
            (MedioPago::Debito, 1, 1224),
            (MedioPago::Credito, 0, 0),
            (MedioPago::Transferencia, 1, 1500),
            (MedioPago::Fiado, 0, 0),
        ]
    );
    assert_eq!((r.efectivo.ingresos, r.efectivo.retiros), (10_000, 5_000));
    assert_eq!(r.movimientos.len(), 2);
    assert_eq!(r.movimientos[1].motivo, "Pago al panadero");
    // 20.000 + 4.220 + 10.000 − 5.000 = 29.220: el débito y la transferencia no son efectivo.
    assert_eq!(r.efectivo.esperado, 29_220);
    assert!(r.turno.abierto);
    assert_eq!(r.turno.efectivo_esperado, None);
}

#[tokio::test]
async fn movimientos_validan_monto_motivo_y_efectivo_disponible() {
    let pool = db::abrir_en_memoria().await.unwrap();
    assert!(matches!(
        movimiento(&pool, TipoMovimiento::Ingreso, 1000, "Cambio").await,
        Err(Error::Validacion(m)) if m.starts_with("Abre la caja")
    ));
    turnos::abrir(&pool, 10_000).await.unwrap();
    assert!(movimiento(&pool, TipoMovimiento::Ingreso, 0, "Cambio")
        .await
        .is_err());
    assert!(movimiento(&pool, TipoMovimiento::Ingreso, -5, "Cambio")
        .await
        .is_err());
    assert!(matches!(
        movimiento(&pool, TipoMovimiento::Ingreso, 1000, "   ").await,
        Err(Error::Validacion(m)) if m.contains("motivo")
    ));
    // No se retira más de lo que debería haber.
    assert!(matches!(
        movimiento(&pool, TipoMovimiento::Retiro, 10_001, "Banco").await,
        Err(Error::Validacion(m)) if m.contains("$10.000")
    ));
    movimiento(&pool, TipoMovimiento::Retiro, 10_000, "Banco")
        .await
        .unwrap();
}

#[tokio::test]
async fn el_retiro_pide_pin_y_el_ingreso_no() {
    let pool = db::abrir_en_memoria().await.unwrap();
    seguridad::crear_pin(&pool, "4821").await.unwrap();
    turnos::abrir(&pool, 10_000).await.unwrap();
    let control = ControlPin::new();

    turnos::registrar_movimiento(
        &pool,
        &control,
        TipoMovimiento::Ingreso,
        500,
        "Cambio",
        None,
    )
    .await
    .unwrap();
    assert!(matches!(
        turnos::registrar_movimiento(&pool, &control, TipoMovimiento::Retiro, 500, "Banco", None)
            .await,
        Err(Error::Pin(FalloPin::Requerido))
    ));
    assert!(matches!(
        turnos::registrar_movimiento(
            &pool,
            &control,
            TipoMovimiento::Retiro,
            500,
            "Banco",
            Some("1111")
        )
        .await,
        Err(Error::Pin(FalloPin::Incorrecto { .. }))
    ));
    turnos::registrar_movimiento(
        &pool,
        &control,
        TipoMovimiento::Retiro,
        500,
        "Banco",
        Some("4821"),
    )
    .await
    .unwrap();

    // Cerrar la caja no está protegido por defecto.
    turnos::cerrar(&pool, &control, 10_000, None, None, None)
        .await
        .unwrap();
}

#[tokio::test]
async fn cerrar_la_caja_pide_pin_si_esta_protegido() {
    let pool = db::abrir_en_memoria().await.unwrap();
    seguridad::crear_pin(&pool, "4821").await.unwrap();
    let control = ControlPin::new();
    let mut protegidas = seguridad::estado(&pool).await.unwrap().protegidas;
    protegidas.cerrar_caja = true;
    seguridad::guardar_protegidas(&pool, &control, protegidas, Some("4821"))
        .await
        .unwrap();
    turnos::abrir(&pool, 0).await.unwrap();

    assert!(matches!(
        turnos::cerrar(&pool, &control, 0, None, None, None).await,
        Err(Error::Pin(FalloPin::Requerido))
    ));
    assert!(turnos::estado_caja(&pool).await.unwrap().turno.is_some());
    turnos::cerrar(&pool, &control, 0, None, None, Some("4821"))
        .await
        .unwrap();
}

#[tokio::test]
async fn cierra_con_sobrante_faltante_y_conteo_por_denominacion() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let bebida = comun::crear(&pool, "B", "Bebida", 1500, 100).await;
    let control = ControlPin::new();

    // Sobra: esperado 20.000 + 3.000 = 23.000, contado 23.100.
    let t = turnos::abrir(&pool, 20_000).await.unwrap();
    vender(&pool, bebida.id, 2000, MedioPago::Efectivo).await;
    let conteo = [
        Conteo {
            denominacion: 20_000,
            cantidad: 1,
        },
        Conteo {
            denominacion: 1_000,
            cantidad: 3,
        },
        Conteo {
            denominacion: 100,
            cantidad: 1,
        },
        Conteo {
            denominacion: 50,
            cantidad: 0,
        },
    ];
    let r = turnos::cerrar(
        &pool,
        &control,
        23_100,
        Some(&conteo),
        Some("  Sobran 100  "),
        None,
    )
    .await
    .unwrap();
    assert!(!r.turno.abierto && r.turno.cerrado_en.is_some());
    assert_eq!(r.turno.efectivo_esperado, Some(23_000));
    assert_eq!(r.turno.efectivo_contado, Some(23_100));
    assert_eq!(r.turno.diferencia, Some(100));
    assert_eq!(r.turno.nota_cierre.as_deref(), Some("Sobran 100"));
    // Se guarda sin los ceros, de mayor a menor.
    assert_eq!(r.turno.detalle_conteo.as_deref(), Some(&conteo[..3]));

    // El turno cerrado no cambia aunque se venda en el siguiente.
    let siguiente = turnos::abrir(&pool, 23_000).await.unwrap();
    vender(&pool, bebida.id, 1000, MedioPago::Efectivo).await;
    let otra_vez = turnos::resumen(&pool, t.id).await.unwrap();
    assert_eq!(otra_vez, r);

    // Falta: esperado 23.000 + 1.500 = 24.500, contado 24.000 (monto directo, sin detalle).
    let r = turnos::cerrar(&pool, &control, 24_000, None, None, None)
        .await
        .unwrap();
    assert_eq!(r.turno.id, siguiente.id);
    assert_eq!(r.turno.diferencia, Some(-500));
    assert_eq!(r.turno.detalle_conteo, None);

    // Sin caja ya no se vende.
    assert!(
        ventas::confirmar(&pool, &pedido(bebida.id, 1000, MedioPago::Debito))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn el_conteo_debe_sumar_lo_contado() {
    let pool = db::abrir_en_memoria().await.unwrap();
    turnos::abrir(&pool, 0).await.unwrap();
    let conteo = [Conteo {
        denominacion: 10_000,
        cantidad: 2,
    }];
    assert!(matches!(
        turnos::cerrar(&pool, &ControlPin::new(), 25_000, Some(&conteo), None, None).await,
        Err(Error::Validacion(m)) if m.contains("$20.000") && m.contains("$25.000")
    ));
    assert!(turnos::estado_caja(&pool).await.unwrap().turno.is_some());
}

#[tokio::test]
async fn lista_los_turnos_del_mas_nuevo_al_mas_antiguo() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let control = ControlPin::new();
    let a = turnos::abrir(&pool, 0).await.unwrap();
    turnos::cerrar(&pool, &control, 0, None, None, None)
        .await
        .unwrap();
    let b = turnos::abrir(&pool, 0).await.unwrap();
    let lista = turnos::listar(&pool, 10, 0).await.unwrap();
    assert_eq!(
        lista.iter().map(|t| (t.id, t.abierto)).collect::<Vec<_>>(),
        [(b.id, true), (a.id, false)]
    );
}

#[tokio::test]
async fn el_cierre_sale_en_la_termica_y_en_pdf() {
    use pos_core::servicios::config::{self, Config};
    use pos_core::servicios::{impresion, negocio};

    let pool = db::abrir_en_memoria().await.unwrap();
    let control = ControlPin::new();
    let bebida = comun::crear(&pool, "B", "Bebida", 1500, 100).await;
    let t = turnos::abrir(&pool, 10_000).await.unwrap();
    vender(&pool, bebida.id, 2000, MedioPago::Efectivo).await;
    turnos::cerrar(&pool, &control, 13_000, None, None, None)
        .await
        .unwrap();

    // Sin nombre del negocio no se imprime, igual que la boleta.
    assert!(impresion::trabajo_cierre(&pool, t.id).await.is_err());
    negocio::guardar(
        &pool,
        &control,
        None,
        &negocio::DatosNegocio {
            nombre: "Almacén".into(),
            rut: None,
            direccion: None,
            telefono: None,
            mensaje_pie: String::new(),
        },
    )
    .await
    .unwrap();
    let c = Config {
        impresora: Some("Archivo (pruebas)".into()),
        ..Config::default()
    };
    config::guardar(&pool, &control, None, &c).await.unwrap();

    let trabajo = impresion::trabajo_cierre(&pool, t.id).await.unwrap();
    assert_eq!(trabajo.documento, format!("cierre-{:06}", t.id));
    let contiene = |texto: &[u8]| trabajo.bytes.windows(texto.len()).any(|w| w == texto);
    assert!(contiene(b"$13.000"));
    assert!(contiene(b"CAJA CUADRADA"));

    let pdf = impresion::preparar_pdf_cierre(&pool, t.id).await.unwrap();
    assert_eq!(pdf.nombre_archivo(), format!("cierre-{:06}.pdf", t.id));
    assert!(pdf.generar().unwrap().starts_with(b"%PDF"));
}
