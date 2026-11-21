// Clientes y venta fiada: cliente obligatorio, límite de crédito con y sin PIN, efectivo del turno,
// boleta con la deuda y devoluciones que nunca dejan saldo negativo.

mod comun;

use pos_core::servicios::clientes::{self, DatosCliente};
use pos_core::servicios::devoluciones::{self, ItemDevuelto};
use pos_core::servicios::historial;
use pos_core::servicios::seguridad::{self, ControlPin};
use pos_core::servicios::turnos;
use pos_core::servicios::ventas::{self, ItemPedido, MedioPago, NuevaVenta, VentaConfirmada};
use pos_core::{db, Error, FalloPin};
use sqlx::SqlitePool;

fn cliente(nombre: &str, limite: Option<i64>) -> DatosCliente {
    DatosCliente {
        nombre: nombre.into(),
        telefono: Some("+56 9 1111 2222".into()),
        rut: None,
        direccion: None,
        nota: None,
        limite_credito: limite,
    }
}

fn fiado(producto_id: i64, unidades: i64, cliente_id: Option<i64>) -> NuevaVenta {
    NuevaVenta {
        items: vec![ItemPedido {
            producto_id,
            cantidad_mil: unidades * 1000,
        }],
        medio_pago: MedioPago::Fiado,
        monto_recibido: None,
        descuento: 0,
        cliente_id,
    }
}

async fn fiar(
    pool: &SqlitePool,
    producto_id: i64,
    unidades: i64,
    cliente_id: i64,
) -> VentaConfirmada {
    ventas::confirmar(pool, &fiado(producto_id, unidades, Some(cliente_id)))
        .await
        .unwrap()
}

#[tokio::test]
async fn no_se_fia_sin_cliente_activo() {
    let pool = comun::con_caja().await;
    let p = comun::crear(&pool, "B", "Bebida", 1000, 10).await;
    assert!(matches!(
        ventas::confirmar(&pool, &fiado(p.id, 1, None)).await,
        Err(Error::Validacion(m)) if m.contains("cliente")
    ));
    assert!(ventas::confirmar(&pool, &fiado(p.id, 1, Some(999)))
        .await
        .is_err());
    let rosa = clientes::crear(&pool, &cliente("Rosa", None))
        .await
        .unwrap();
    clientes::cambiar_activo(&pool, rosa.id, false)
        .await
        .unwrap();
    assert!(ventas::confirmar(&pool, &fiado(p.id, 1, Some(rosa.id)))
        .await
        .is_err());
    clientes::cambiar_activo(&pool, rosa.id, true)
        .await
        .unwrap();
    let v = fiar(&pool, p.id, 2, rosa.id).await;
    assert_eq!(
        (v.redondeo, v.cliente.as_deref(), v.deuda_cliente),
        (0, Some("Rosa"), Some(2000))
    );
}

#[tokio::test]
async fn busca_clientes_y_no_desactiva_si_debe() {
    let pool = comun::con_caja().await;
    let p = comun::crear(&pool, "B", "Bebida", 1000, 10).await;
    let mut d = cliente("Rosa Martínez", Some(10_000));
    d.rut = Some("12345678-5".into());
    let rosa = clientes::crear(&pool, &d).await.unwrap();
    assert_eq!(rosa.rut.as_deref(), Some("12.345.678-5"));
    clientes::crear(&pool, &cliente("Juan Pérez", None))
        .await
        .unwrap();
    for texto in ["rosa", "1111", "12.345.678", "123456785"] {
        let r = clientes::buscar(&pool, texto, 10).await.unwrap();
        assert_eq!(r.len(), if texto == "1111" { 2 } else { 1 }, "{texto}");
    }
    assert!(clientes::crear(&pool, &cliente("  ", None)).await.is_err());

    fiar(&pool, p.id, 3, rosa.id).await;
    let r = clientes::obtener(&pool, rosa.id).await.unwrap();
    assert_eq!((r.deuda, r.disponible), (3000, Some(7000)));
    assert!(matches!(
        clientes::cambiar_activo(&pool, rosa.id, false).await,
        Err(Error::Validacion(m)) if m.contains("$3.000")
    ));
}

#[tokio::test]
async fn el_limite_de_credito_se_respeta_y_se_autoriza_con_pin() {
    let pool = comun::con_caja().await;
    let p = comun::crear(&pool, "B", "Bebida", 1000, 50).await;
    let rosa = clientes::crear(&pool, &cliente("Rosa", Some(5000)))
        .await
        .unwrap();
    let control = ControlPin::new();
    fiar(&pool, p.id, 3, rosa.id).await;

    // 3.000 + 3.000 pasa del límite de 5.000.
    let pedido = fiado(p.id, 3, Some(rosa.id));
    assert!(matches!(
        ventas::confirmar(&pool, &pedido).await,
        Err(Error::Validacion(m)) if m.contains("límite") && m.contains("$2.000")
    ));
    // Justo al límite sí.
    fiar(&pool, p.id, 2, rosa.id).await;
    // Sin PIN creado, autorizar basta.
    ventas::confirmar_autorizada(&pool, &control, &fiado(p.id, 1, Some(rosa.id)), true, None)
        .await
        .unwrap();
    // Con PIN: lo pide y con el correcto pasa.
    seguridad::crear_pin(&pool, "4821").await.unwrap();
    assert!(matches!(
        ventas::confirmar_autorizada(&pool, &control, &pedido, true, None).await,
        Err(Error::Pin(FalloPin::Requerido))
    ));
    let v = ventas::confirmar_autorizada(&pool, &control, &pedido, true, Some("4821"))
        .await
        .unwrap();
    assert_eq!(v.deuda_cliente, Some(9000));
    // Sin autorizar sigue rechazada aunque haya PIN.
    assert!(ventas::confirmar(&pool, &pedido).await.is_err());
}

#[tokio::test]
async fn la_venta_fiada_no_altera_el_efectivo_y_la_boleta_dice_la_deuda() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let p = comun::crear_kg(&pool, "Q", "Queso", 2300, 5000).await;
    let t = turnos::abrir(&pool, 10_000).await.unwrap();
    let rosa = clientes::crear(&pool, &cliente("Rosa", None))
        .await
        .unwrap();
    // 0,532 kg = $1.224: fiado no se redondea.
    let v = ventas::confirmar(
        &pool,
        &NuevaVenta {
            items: vec![ItemPedido {
                producto_id: p.id,
                cantidad_mil: 532,
            }],
            medio_pago: MedioPago::Fiado,
            monto_recibido: None,
            descuento: 0,
            cliente_id: Some(rosa.id),
        },
    )
    .await
    .unwrap();
    assert_eq!((v.total, v.redondeo), (1224, 0));
    let r = turnos::resumen(&pool, t.id).await.unwrap();
    assert_eq!(r.efectivo.esperado, 10_000);
    assert_eq!(
        r.ventas
            .iter()
            .find(|x| x.medio == MedioPago::Fiado)
            .unwrap()
            .monto,
        1224
    );

    sqlx::query("UPDATE negocio SET nombre = 'Almacén'")
        .execute(&pool)
        .await
        .unwrap();
    let b = pos_core::boleta::armar(&pool, v.id).await.unwrap();
    assert_eq!(
        (b.cliente.as_deref(), b.deuda_cliente),
        (Some("Rosa"), Some(1224))
    );
    let texto = pos_core::boleta::texto::a_texto(&pos_core::boleta::texto::renderizar(&b, 48), 48);
    assert!(texto.contains("FIADO a Rosa"), "{texto}");
    assert!(texto.contains("Deuda total del cliente"), "{texto}");
}

#[tokio::test]
async fn la_devolucion_de_una_fiada_nunca_deja_saldo_negativo() {
    let pool = comun::con_caja().await;
    let p = comun::crear(&pool, "B", "Bebida", 1000, 50).await;
    let rosa = clientes::crear(&pool, &cliente("Rosa", None))
        .await
        .unwrap();
    let a = fiar(&pool, p.id, 5, rosa.id).await; // 5.000
    let b = fiar(&pool, p.id, 2, rosa.id).await; // 2.000
                                                 // A tiene 4.000 abonados (de antes de los pagos): debe 1.000.
    sqlx::query("INSERT INTO abonos (venta_id, monto, medio_pago) VALUES (?, 4000, 'efectivo')")
        .bind(a.id)
        .execute(&pool)
        .await
        .unwrap();
    let item = historial::detalle(&pool, a.id).await.unwrap().lineas[0].id;

    // Devuelve 4 bebidas ($4.000): 1.000 bajan la deuda de A, 2.000 pasan a B y 1.000 se devuelven.
    let r = devoluciones::registrar(
        &pool,
        &ControlPin::new(),
        a.id,
        &[ItemDevuelto {
            venta_item_id: item,
            cantidad_mil: 4000,
            vuelve_a_stock: true,
        }],
        Some(MedioPago::Efectivo),
        "No le gustó",
        None,
    )
    .await
    .unwrap();
    assert_eq!(r.monto, 4000);
    let d = &r.venta.devoluciones[0];
    assert_eq!(d.medio_reembolso, Some(MedioPago::Efectivo));
    assert_eq!(r.venta.saldo, Some(0));
    assert_eq!(
        historial::detalle(&pool, b.id).await.unwrap().saldo,
        Some(0)
    );
    assert_eq!(clientes::obtener(&pool, rosa.id).await.unwrap().deuda, 0);
    // A y B quedaron saldadas ('pagada'), y del turno salió solo el efectivo devuelto.
    let estados: Vec<String> = sqlx::query_scalar("SELECT estado FROM ventas ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(estados, ["pagada", "pagada"]);
    let turno = turnos::estado_caja(&pool).await.unwrap().turno.unwrap();
    assert_eq!(
        turnos::resumen(&pool, turno.id)
            .await
            .unwrap()
            .efectivo
            .reembolsos,
        1000
    );
}
