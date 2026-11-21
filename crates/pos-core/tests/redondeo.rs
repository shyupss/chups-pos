mod comun;

use pos_core::servicios::config;
use pos_core::servicios::seguridad::ControlPin;
use pos_core::servicios::ventas::{self, ItemPedido, MedioPago, NuevaVenta, VentaConfirmada};
use pos_core::Error;
use std::sync::atomic::{AtomicU32, Ordering};

use sqlx::SqlitePool;

static SIGUIENTE_CODIGO: AtomicU32 = AtomicU32::new(1);

/// Vende 1 unidad de un producto que cuesta exactamente `precio`.
async fn vender(pool: &SqlitePool, precio: i64, medio: MedioPago) -> VentaConfirmada {
    let codigo = format!("P{}", SIGUIENTE_CODIGO.fetch_add(1, Ordering::Relaxed));
    let p = comun::crear(pool, &codigo, &format!("Producto {precio}"), precio, 0).await;
    ventas::confirmar(
        pool,
        &NuevaVenta {
            items: vec![ItemPedido {
                producto_id: p.id,
                cantidad_mil: 1000,
            }],
            medio_pago: medio,
            monto_recibido: None,
            descuento: 0,
            cliente_id: None,
        },
    )
    .await
    .unwrap()
}

async fn redondeo_guardado(pool: &SqlitePool, venta_id: i64) -> (i64, i64, i64) {
    sqlx::query_as("SELECT subtotal, redondeo, total FROM ventas WHERE id = ?")
        .bind(venta_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn efectivo_se_redondea_a_la_decena() {
    let pool = comun::con_caja().await;
    assert!(
        config::obtener(&pool).await.unwrap().redondeo_efectivo,
        "activo por defecto"
    );

    // (subtotal, redondeo, total cobrado)
    for (precio, redondeo, total) in [
        (1224, -4, 1220),
        (1226, 4, 1230),
        (1225, -5, 1220),
        (1220, 0, 1220),
    ] {
        let v = vender(&pool, precio, MedioPago::Efectivo).await;
        assert_eq!((v.subtotal, v.redondeo, v.total), (precio, redondeo, total));
        assert_eq!(
            v.vuelto,
            Some(0),
            "sin monto recibido paga justo el total redondeado"
        );
        assert_eq!(
            redondeo_guardado(&pool, v.id).await,
            (precio, redondeo, total)
        );
    }
}

#[tokio::test]
async fn el_vuelto_se_calcula_sobre_el_total_redondeado() {
    let pool = comun::con_caja().await;
    let p = comun::crear(&pool, "1", "Queque", 1226, 0).await;
    let v = ventas::confirmar(
        &pool,
        &NuevaVenta {
            items: vec![ItemPedido {
                producto_id: p.id,
                cantidad_mil: 1000,
            }],
            medio_pago: MedioPago::Efectivo,
            monto_recibido: Some(2000),
            descuento: 0,
            cliente_id: None,
        },
    )
    .await
    .unwrap();
    assert_eq!((v.total, v.vuelto), (1230, Some(770)));

    // Pagar el subtotal sin redondear ya no alcanza.
    let corto = ventas::confirmar(
        &pool,
        &NuevaVenta {
            items: vec![ItemPedido {
                producto_id: p.id,
                cantidad_mil: 1000,
            }],
            medio_pago: MedioPago::Efectivo,
            monto_recibido: Some(1226),
            descuento: 0,
            cliente_id: None,
        },
    )
    .await;
    assert!(matches!(corto, Err(Error::Validacion(_))), "{corto:?}");
}

#[tokio::test]
async fn tarjetas_y_transferencia_no_se_redondean() {
    let pool = comun::con_caja().await;
    for medio in [
        MedioPago::Debito,
        MedioPago::Credito,
        MedioPago::Transferencia,
    ] {
        let v = vender(&pool, 1224, medio).await;
        assert_eq!((v.redondeo, v.total), (0, 1224), "{medio:?}");
    }
}

#[tokio::test]
async fn con_redondeo_desactivado_no_se_redondea() {
    let pool = comun::con_caja().await;
    cambiar_redondeo(&pool, false).await.unwrap();
    assert!(!config::obtener(&pool).await.unwrap().redondeo_efectivo);

    let v = vender(&pool, 1224, MedioPago::Efectivo).await;
    assert_eq!((v.redondeo, v.total), (0, 1224));

    let valor: String =
        sqlx::query_scalar("SELECT valor FROM config WHERE clave = 'redondeo_efectivo'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(valor, "0");

    cambiar_redondeo(&pool, true).await.unwrap();
    let v = vender(&pool, 1226, MedioPago::Efectivo).await;
    assert_eq!((v.redondeo, v.total), (4, 1230));
}

#[tokio::test]
async fn venta_por_peso_en_efectivo_se_redondea() {
    let pool = comun::con_caja().await;
    let queso = comun::crear_kg(&pool, "Q", "Queso", 2300, 10_000).await;
    let v = ventas::confirmar(
        &pool,
        &NuevaVenta {
            items: vec![ItemPedido {
                producto_id: queso.id,
                cantidad_mil: 532,
            }],
            medio_pago: MedioPago::Efectivo,
            monto_recibido: Some(5000),
            descuento: 0,
            cliente_id: None,
        },
    )
    .await
    .unwrap();
    // 532 g a $2.300/kg = $1.224 → se cobra $1.220.
    assert_eq!(
        (v.subtotal, v.redondeo, v.total, v.vuelto),
        (1224, -4, 1220, Some(3780))
    );
}

async fn cambiar_redondeo(pool: &SqlitePool, activo: bool) -> Result<(), Error> {
    let mut c = config::obtener(pool).await?;
    c.redondeo_efectivo = activo;
    config::guardar(pool, &ControlPin::new(), None, &c)
        .await
        .map(|_| ())
}
