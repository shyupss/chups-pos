// Pagos de fiados: reparto de la más antigua a la más nueva o a las ventas marcadas, redondeo y
// vuelto en efectivo, tope en la deuda, anulación con PIN y su efecto en la caja, y el cierre de
// un turno con abonos y una compra pagada con la caja.

mod comun;

use pos_core::servicios::anulacion;
use pos_core::servicios::clientes::{self, DatosCliente};
use pos_core::servicios::compras::{self, LineaCompra, NuevaCompra};
use pos_core::servicios::fiados::{self, NuevoPagoCliente};
use pos_core::servicios::historial;
use pos_core::servicios::seguridad::{self, ControlPin};
use pos_core::servicios::turnos;
use pos_core::servicios::ventas::{self, ItemPedido, MedioPago, NuevaVenta};
use pos_core::{db, Error, FalloPin};
use sqlx::SqlitePool;

async fn cliente(pool: &SqlitePool, nombre: &str) -> i64 {
    clientes::crear(
        pool,
        &DatosCliente {
            nombre: nombre.into(),
            telefono: None,
            rut: None,
            direccion: None,
            nota: None,
            limite_credito: None,
        },
    )
    .await
    .unwrap()
    .id
}

async fn fiar(pool: &SqlitePool, producto_id: i64, unidades: i64, cliente_id: i64) -> i64 {
    ventas::confirmar(
        pool,
        &NuevaVenta {
            items: vec![ItemPedido {
                producto_id,
                cantidad_mil: unidades * 1000,
            }],
            medio_pago: MedioPago::Fiado,
            monto_recibido: None,
            descuento: 0,
            cliente_id: Some(cliente_id),
        },
    )
    .await
    .unwrap()
    .id
}

fn pago(cliente_id: i64, monto: i64, medio: MedioPago) -> NuevoPagoCliente {
    NuevoPagoCliente {
        cliente_id,
        monto,
        medio_pago: medio,
        ventas: vec![],
        monto_recibido: None,
        nota: None,
    }
}

async fn con_nombre(pool: &SqlitePool) {
    sqlx::query("UPDATE negocio SET nombre = 'Almacén'")
        .execute(pool)
        .await
        .unwrap();
}

async fn saldo(pool: &SqlitePool, venta_id: i64) -> i64 {
    historial::detalle(pool, venta_id)
        .await
        .unwrap()
        .saldo
        .unwrap()
}

async fn estado(pool: &SqlitePool, venta_id: i64) -> String {
    sqlx::query_scalar("SELECT estado FROM ventas WHERE id = ?")
        .bind(venta_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn el_pago_salda_de_la_mas_antigua_a_la_mas_nueva() {
    let pool = comun::con_caja().await;
    let p = comun::crear(&pool, "B", "Bebida", 1000, 50).await;
    let rosa = cliente(&pool, "Rosa").await;
    let a = fiar(&pool, p.id, 3, rosa).await; // 3.000
    let b = fiar(&pool, p.id, 2, rosa).await; // 2.000
    let c = fiar(&pool, p.id, 4, rosa).await; // 4.000

    let r = fiados::registrar_pago(&pool, &pago(rosa, 4_000, MedioPago::Debito))
        .await
        .unwrap();
    assert_eq!(r.deuda_restante, 5_000);
    let abonos: Vec<(i64, i64)> = r
        .pago
        .abonos
        .iter()
        .map(|x| (x.venta_id, x.monto))
        .collect();
    assert_eq!(abonos, [(a, 3_000), (b, 1_000)]);
    assert_eq!(
        (estado(&pool, a).await, saldo(&pool, a).await),
        ("pagada".into(), 0)
    );
    assert_eq!(
        (estado(&pool, b).await, saldo(&pool, b).await),
        ("fiada".into(), 1_000)
    );
    assert_eq!(saldo(&pool, c).await, 4_000);

    // Marcando la C, el pago va primero a ella y lo que sobra a la B.
    let mut marcado = pago(rosa, 4_500, MedioPago::Transferencia);
    marcado.ventas = vec![c];
    let r = fiados::registrar_pago(&pool, &marcado).await.unwrap();
    let abonos: Vec<(i64, i64)> = r
        .pago
        .abonos
        .iter()
        .map(|x| (x.venta_id, x.monto))
        .collect();
    assert_eq!(abonos, [(c, 4_000), (b, 500)]);
    assert_eq!(r.deuda_restante, 500);

    // No se paga más de lo que debe, ni con un monto inválido, ni una venta de otro.
    assert!(matches!(
        fiados::registrar_pago(&pool, &pago(rosa, 501, MedioPago::Debito)).await,
        Err(Error::Validacion(m)) if m.contains("mayor que la deuda")
    ));
    assert!(
        fiados::registrar_pago(&pool, &pago(rosa, 0, MedioPago::Debito))
            .await
            .is_err()
    );
    assert!(
        fiados::registrar_pago(&pool, &pago(rosa, 100, MedioPago::Fiado))
            .await
            .is_err()
    );
    let juan = cliente(&pool, "Juan").await;
    let j = fiar(&pool, p.id, 1, juan).await;
    let mut ajena = pago(rosa, 100, MedioPago::Debito);
    ajena.ventas = vec![j];
    assert!(fiados::registrar_pago(&pool, &ajena).await.is_err());

    // "Paga todo": la deuda exacta deja todo saldado.
    let r = fiados::registrar_pago(&pool, &pago(rosa, 500, MedioPago::Debito))
        .await
        .unwrap();
    assert_eq!(r.deuda_restante, 0);
    let f = fiados::ficha(&pool, rosa).await.unwrap();
    assert!(f.pendientes.is_empty());
    assert_eq!(f.pagadas.len(), 3);
    assert_eq!(f.pagos.len(), 3);
    let deudores = fiados::listar_deudores(&pool, "", false, 50, 0)
        .await
        .unwrap();
    assert_eq!(deudores.clientes.len(), 1);
    assert_eq!(deudores.clientes[0].nombre, "Juan");
    assert_eq!(
        (deudores.total_adeudado, deudores.clientes_con_deuda),
        (Some(1_000), Some(1))
    );
}

#[tokio::test]
async fn el_pago_en_efectivo_se_redondea_entra_a_la_caja_y_da_vuelto() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let p = comun::crear(&pool, "B", "Bebida", 12_345, 50).await;
    let rosa = cliente(&pool, "Rosa").await;
    // Sin caja se puede fiar (no mueve efectivo), pero no recibir efectivo.
    let t = turnos::abrir(&pool, 10_000).await.unwrap();
    let v = fiar(&pool, p.id, 1, rosa).await;
    let id_turno = t.id;
    turnos::cerrar(&pool, &ControlPin::new(), 10_000, None, None, None)
        .await
        .unwrap();
    assert!(
        fiados::registrar_pago(&pool, &pago(rosa, 12_345, MedioPago::Efectivo))
            .await
            .is_err()
    );
    // Con débito no hace falta la caja.
    fiados::registrar_pago(&pool, &pago(rosa, 45, MedioPago::Debito))
        .await
        .unwrap();
    assert_ne!(id_turno, 0);

    let t = turnos::abrir(&pool, 0).await.unwrap();
    let mut efectivo = pago(rosa, 12_300, MedioPago::Efectivo);
    // 12.300 no se redondea; pagó con 15.000.
    efectivo.monto_recibido = Some(15_000);
    let r = fiados::registrar_pago(&pool, &efectivo).await.unwrap();
    assert_eq!(
        (r.pago.redondeo, r.pago.vuelto, r.deuda_restante),
        (0, Some(2_700), 0)
    );
    assert_eq!(estado(&pool, v).await, "pagada");
    let resumen = turnos::resumen(&pool, t.id).await.unwrap();
    assert_eq!(resumen.efectivo.abonos, 12_300);
    assert_eq!(resumen.efectivo.esperado, 12_300);

    // Un monto que no termina en 0 se redondea como al cobrar: 1.234 → paga 1.230.
    let w = fiar(&pool, p.id, 1, rosa).await;
    let mut chico = pago(rosa, 1_234, MedioPago::Efectivo);
    chico.monto_recibido = Some(1_000);
    assert!(fiados::registrar_pago(&pool, &chico).await.is_err());
    chico.monto_recibido = None;
    let r = fiados::registrar_pago(&pool, &chico).await.unwrap();
    assert_eq!(
        (r.pago.monto, r.pago.redondeo, r.pago.vuelto),
        (1_234, -4, Some(0))
    );
    assert_eq!(saldo(&pool, w).await, 12_345 - 1_234);
    let resumen = turnos::resumen(&pool, t.id).await.unwrap();
    assert_eq!(resumen.efectivo.abonos, 12_300 + 1_230);
}

#[tokio::test]
async fn anular_un_pago_pide_pin_vuelve_a_deber_y_devuelve_el_efectivo() {
    let pool = comun::con_caja().await;
    let control = ControlPin::new();
    seguridad::crear_pin(&pool, "4321").await.unwrap();
    let p = comun::crear(&pool, "B", "Bebida", 1000, 50).await;
    let rosa = cliente(&pool, "Rosa").await;
    let a = fiar(&pool, p.id, 2, rosa).await;
    let b = fiar(&pool, p.id, 3, rosa).await;
    let r = fiados::registrar_pago(&pool, &pago(rosa, 3_000, MedioPago::Efectivo))
        .await
        .unwrap();
    assert_eq!(estado(&pool, a).await, "pagada");
    let turno = turnos::estado_caja(&pool).await.unwrap().turno.unwrap().id;
    assert_eq!(
        turnos::resumen(&pool, turno)
            .await
            .unwrap()
            .efectivo
            .esperado,
        3_000
    );

    assert!(matches!(
        fiados::anular_pago(&pool, &control, r.pago.id, "Se ingresó mal", None).await,
        Err(Error::Pin(FalloPin::Requerido))
    ));
    assert!(
        fiados::anular_pago(&pool, &control, r.pago.id, "", Some("4321"))
            .await
            .is_err()
    );
    let anulado = fiados::anular_pago(&pool, &control, r.pago.id, "Se ingresó mal", Some("4321"))
        .await
        .unwrap();
    assert!(anulado.pago.anulado_en.is_some());
    assert_eq!(anulado.deuda_restante, 5_000);
    assert_eq!(
        (estado(&pool, a).await, saldo(&pool, a).await),
        ("fiada".into(), 2_000)
    );
    assert_eq!(saldo(&pool, b).await, 3_000);
    let resumen = turnos::resumen(&pool, turno).await.unwrap();
    assert_eq!(resumen.efectivo.reembolsos, 3_000);
    assert_eq!(resumen.efectivo.esperado, 0);
    assert_eq!(resumen.pagos_anulados.len(), 1);
    assert!(
        fiados::anular_pago(&pool, &control, r.pago.id, "Otra vez", Some("4321"))
            .await
            .is_err()
    );
    // El pago anulado sigue en la ficha (nada se borra) y su comprobante dice ANULADO.
    let f = fiados::ficha(&pool, rosa).await.unwrap();
    assert_eq!(f.pagos.len(), 1);
    con_nombre(&pool).await;
    let c = pos_core::boleta::pago::armar(&pool, r.pago.id)
        .await
        .unwrap();
    assert!(c.anulado.is_some());

    // Si la venta que pagó se anula después, el pago ya no se puede deshacer.
    let r = fiados::registrar_pago(&pool, &pago(rosa, 1_000, MedioPago::Debito))
        .await
        .unwrap();
    anulacion::anular(
        &pool,
        &control,
        a,
        "Error de caja",
        Some(MedioPago::Debito),
        Some("4321"),
    )
    .await
    .unwrap();
    assert!(matches!(
        fiados::anular_pago(&pool, &control, r.pago.id, "Se ingresó mal", Some("4321")).await,
        Err(Error::Validacion(m)) if m.contains("ya no se puede anular")
    ));
}

#[tokio::test]
async fn anular_una_fiada_con_pago_parcial_pasa_lo_abonado_a_otra_deuda() {
    let pool = comun::con_caja().await;
    let control = ControlPin::new();
    let p = comun::crear(&pool, "B", "Bebida", 1000, 50).await;
    let rosa = cliente(&pool, "Rosa").await;
    let a = fiar(&pool, p.id, 2, rosa).await; // 2.000
    let b = fiar(&pool, p.id, 5, rosa).await; // 5.000
    fiados::registrar_pago(&pool, &pago(rosa, 1_500, MedioPago::Debito))
        .await
        .unwrap();
    // A debe 500; al anularla, los 1.500 abonados bajan la deuda de B.
    anulacion::anular(&pool, &control, a, "Se equivocó de cliente", None, None)
        .await
        .unwrap();
    assert_eq!(saldo(&pool, b).await, 3_500);
    assert_eq!(clientes::obtener(&pool, rosa).await.unwrap().deuda, 3_500);
}

#[tokio::test]
async fn el_cierre_cuenta_abonos_y_la_compra_pagada_con_caja() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let p = comun::crear(&pool, "B", "Bebida", 1000, 50).await;
    let rosa = cliente(&pool, "Rosa").await;
    let t = turnos::abrir(&pool, 20_000).await.unwrap();
    fiar(&pool, p.id, 8, rosa).await;
    // Venta en efectivo de $2.000, pago de fiado en efectivo de $5.000 y otro con débito.
    ventas::confirmar(
        &pool,
        &NuevaVenta {
            items: vec![ItemPedido {
                producto_id: p.id,
                cantidad_mil: 2000,
            }],
            medio_pago: MedioPago::Efectivo,
            monto_recibido: None,
            descuento: 0,
            cliente_id: None,
        },
    )
    .await
    .unwrap();
    fiados::registrar_pago(&pool, &pago(rosa, 5_000, MedioPago::Efectivo))
        .await
        .unwrap();
    fiados::registrar_pago(&pool, &pago(rosa, 1_000, MedioPago::Debito))
        .await
        .unwrap();
    // Compra al proveedor por $7.500 pagada con la caja.
    compras::registrar(
        &pool,
        &ControlPin::new(),
        &NuevaCompra {
            proveedor_id: None,
            documento: None,
            nota: None,
            lineas: vec![LineaCompra {
                producto_id: p.id,
                cantidad_mil: 10_000,
                costo_unitario: 750,
            }],
            pagada_con_caja: true,
        },
        None,
    )
    .await
    .unwrap();

    let r = turnos::cerrar(&pool, &ControlPin::new(), 19_500, None, None, None)
        .await
        .unwrap();
    let e = r.efectivo;
    assert_eq!(
        (e.monto_inicial, e.ventas, e.abonos, e.retiros, e.esperado),
        (20_000, 2_000, 5_000, 7_500, 19_500)
    );
    assert_eq!(r.turno.diferencia, Some(0));
    let abonos: Vec<(MedioPago, i64)> = r.abonos.iter().map(|x| (x.medio, x.monto)).collect();
    assert!(abonos.contains(&(MedioPago::Efectivo, 5_000)));
    assert!(abonos.contains(&(MedioPago::Debito, 1_000)));
    con_nombre(&pool).await;
    let cierre = pos_core::boleta::cierre::armar(&pool, t.id).await.unwrap();
    let texto =
        pos_core::boleta::texto::a_texto(&pos_core::boleta::Documento::lineas(&cierre, 48), 48);
    assert!(texto.contains("ABONOS DE FIADOS"), "{texto}");
    assert!(texto.contains("CAJA CUADRADA"), "{texto}");
}
