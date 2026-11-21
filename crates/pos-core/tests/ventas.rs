mod comun;

use pos_core::servicios::ventas::{self, ItemPedido, MedioPago, NuevaVenta};
use pos_core::{db, Error};

fn venta(items: &[(i64, i64)], medio: MedioPago, recibido: Option<i64>) -> NuevaVenta {
    NuevaVenta {
        items: items
            .iter()
            .map(|&(producto_id, unidades)| ItemPedido {
                producto_id,
                cantidad_mil: unidades * 1000,
            })
            .collect(),
        medio_pago: medio,
        monto_recibido: recibido,
        descuento: 0,
        cliente_id: None,
    }
}

#[tokio::test]
async fn venta_en_efectivo_calcula_vuelto_y_descuenta_stock() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let pan = comun::crear(&pool, "1", "Pan", 150, 100).await;
    let leche = comun::crear(&pool, "2", "Leche", 1190, 10).await;

    // El pan aparece dos veces (dos escaneos): se junta en una línea.
    let v = ventas::confirmar(
        &pool,
        &venta(
            &[(pan.id, 4), (leche.id, 2), (pan.id, 2)],
            MedioPago::Efectivo,
            Some(5000),
        ),
    )
    .await
    .unwrap();

    assert_eq!(v.folio, 1);
    assert_eq!(v.total, 6 * 150 + 2 * 1190);
    assert_eq!(v.vuelto, Some(5000 - 3280));
    assert_eq!(v.lineas.len(), 2);
    assert_eq!(v.lineas[0].nombre, "Pan");
    assert_eq!(v.lineas[0].cantidad_mil, 6000);
    assert_eq!(v.lineas[0].stock_resultante_mil, 94_000);
    assert_eq!(v.lineas[1].stock_resultante_mil, 8000);

    let movimientos: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM movimientos_stock WHERE tipo = 'venta' AND venta_id = ?",
    )
    .bind(v.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(movimientos, 2);
}

#[tokio::test]
async fn folios_son_correlativos_y_tarjeta_no_da_vuelto() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let p = comun::crear(&pool, "1", "Bebida", 1000, 0).await;

    let a = ventas::confirmar(&pool, &venta(&[(p.id, 1)], MedioPago::Debito, None))
        .await
        .unwrap();
    let b = ventas::confirmar(
        &pool,
        &venta(&[(p.id, 1)], MedioPago::Transferencia, Some(99)),
    )
    .await
    .unwrap();

    assert_eq!((a.folio, b.folio), (1, 2));
    assert_eq!((b.monto_recibido, b.vuelto), (Some(1000), Some(0)));
    // Sin stock previo, el stock queda negativo en vez de bloquear la venta.
    assert_eq!(b.lineas[0].stock_resultante_mil, -2000);
}

#[tokio::test]
async fn usa_el_precio_de_la_base_no_el_de_la_interfaz() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let p = comun::crear(&pool, "1", "Queso", 3000, 5).await;
    let mut d = comun::datos(Some("1"), "Queso", 3500, 0);
    d.stock_minimo_mil = 0;
    pos_core::servicios::productos::editar(&pool, p.id, &d)
        .await
        .unwrap();

    let v = ventas::confirmar(&pool, &venta(&[(p.id, 1)], MedioPago::Credito, None))
        .await
        .unwrap();
    assert_eq!(v.total, 3500);
}

#[tokio::test]
async fn rechazos_no_dejan_nada_guardado() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let p = comun::crear(&pool, "1", "Café", 4990, 3).await;

    let casos = [
        venta(&[], MedioPago::Efectivo, None),
        venta(&[(p.id, 0)], MedioPago::Efectivo, None),
        venta(&[(p.id, 1)], MedioPago::Efectivo, Some(1000)),
        venta(&[(p.id, 1)], MedioPago::Fiado, None),
        venta(&[(p.id, 1), (9999, 1)], MedioPago::Debito, None),
    ];
    for caso in &casos {
        let r = ventas::confirmar(&pool, caso).await;
        assert!(
            matches!(r, Err(Error::Validacion(_) | Error::NoEncontrado(_))),
            "{r:?}"
        );
    }

    let mut con_descuento = venta(&[(p.id, 1)], MedioPago::Debito, None);
    con_descuento.descuento = 5000;
    assert!(ventas::confirmar(&pool, &con_descuento).await.is_err());

    let ventas_guardadas: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ventas")
        .fetch_one(&pool)
        .await
        .unwrap();
    let stock: i64 = sqlx::query_scalar("SELECT stock_mil FROM productos WHERE id = ?")
        .bind(p.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((ventas_guardadas, stock), (0, 3000));
}

#[tokio::test]
async fn producto_desactivado_no_se_puede_vender() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let p = comun::crear(&pool, "1", "Viejo", 100, 1).await;
    pos_core::servicios::productos::cambiar_activo(&pool, p.id, false)
        .await
        .unwrap();
    let r = ventas::confirmar(&pool, &venta(&[(p.id, 1)], MedioPago::Debito, None)).await;
    assert!(matches!(r, Err(Error::NoEncontrado(_))));
}
