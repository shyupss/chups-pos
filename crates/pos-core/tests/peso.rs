mod comun;

use pos_core::servicios::ventas::{self, ItemPedido, MedioPago, NuevaVenta};
use pos_core::{db, Error};

fn venta_mil(items: &[(i64, i64)], medio: MedioPago) -> NuevaVenta {
    NuevaVenta {
        items: items
            .iter()
            .map(|&(producto_id, cantidad_mil)| ItemPedido {
                producto_id,
                cantidad_mil,
            })
            .collect(),
        medio_pago: medio,
        monto_recibido: None,
        descuento: 0,
        cliente_id: None,
    }
}

#[tokio::test]
async fn venta_por_peso_redondea_el_subtotal_y_descuenta_gramos() {
    let pool = db::abrir_en_memoria().await.unwrap();
    // $2.300 el kilo, 10 kg en stock.
    let queso = comun::crear_kg(&pool, "Q1", "Queso gauda", 2300, 10_000).await;

    // 532 g: 2300 × 0,532 = 1223,6 → $1.224.
    let v = ventas::confirmar(&pool, &venta_mil(&[(queso.id, 532)], MedioPago::Debito))
        .await
        .unwrap();

    assert_eq!(v.lineas[0].cantidad_mil, 532);
    assert_eq!(v.lineas[0].subtotal, 1224);
    assert_eq!(v.subtotal, 1224);
    assert_eq!(v.lineas[0].stock_resultante_mil, 10_000 - 532);

    let (cantidad_mil, stock_resultante_mil): (i64, i64) = sqlx::query_as(
        "SELECT cantidad_mil, stock_resultante_mil FROM movimientos_stock
         WHERE tipo = 'venta' AND producto_id = ?",
    )
    .bind(queso.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((cantidad_mil, stock_resultante_mil), (-532, 9468));
}

#[tokio::test]
async fn mezcla_productos_por_unidad_y_por_peso() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let pan = comun::crear_kg(&pool, "P1", "Pan amasado", 2490, 0).await;
    let bebida = comun::crear(&pool, "B1", "Bebida", 1500, 10).await;

    // Dos pesadas del mismo pan se juntan en una línea: 300 g + 450 g = 750 g.
    let v = ventas::confirmar(
        &pool,
        &venta_mil(
            &[(pan.id, 300), (bebida.id, 2000), (pan.id, 450)],
            MedioPago::Credito,
        ),
    )
    .await
    .unwrap();

    assert_eq!(v.lineas.len(), 2);
    assert_eq!(v.lineas[0].cantidad_mil, 750);
    assert_eq!(v.lineas[0].subtotal, 1868); // 2490 × 0,75 = 1867,5 → 1868
    assert_eq!(v.lineas[1].subtotal, 3000);
    assert_eq!(v.total, 4868);
    assert_eq!(v.lineas[1].stock_resultante_mil, 8000);
}

#[tokio::test]
async fn rechaza_cantidades_que_no_calzan_con_la_unidad() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let bebida = comun::crear(&pool, "B1", "Bebida", 1500, 10).await;
    let queso = comun::crear_kg(&pool, "Q1", "Queso", 9990, 5000).await;

    let casos = [
        // Decimales en un producto por unidad.
        venta_mil(&[(bebida.id, 1500)], MedioPago::Debito),
        // 0 g.
        venta_mil(&[(queso.id, 0)], MedioPago::Debito),
        // Más de 100 kg.
        venta_mil(&[(queso.id, 100_001)], MedioPago::Debito),
        // Negativo.
        venta_mil(&[(queso.id, -10)], MedioPago::Debito),
    ];
    for caso in &casos {
        let r = ventas::confirmar(&pool, caso).await;
        assert!(matches!(r, Err(Error::Validacion(_))), "{r:?}");
    }

    // Justo en los bordes sí se puede: 1 g y 100 kg.
    for gramos in [1, 100_000] {
        ventas::confirmar(&pool, &venta_mil(&[(queso.id, gramos)], MedioPago::Debito))
            .await
            .unwrap();
    }

    let ventas_guardadas: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ventas")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ventas_guardadas, 2, "los rechazos no guardan nada");
}

#[tokio::test]
async fn producto_por_unidad_exige_stock_entero() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let mut d = comun::datos(Some("X"), "Huevos", 300, 0);
    d.stock_inicial_mil = 1500;
    let r = pos_core::servicios::productos::crear(&pool, &d).await;
    assert!(matches!(r, Err(Error::Validacion(_))), "{r:?}");

    d.unidad = pos_core::repositorios::productos::Unidad::Kg;
    let p = pos_core::servicios::productos::crear(&pool, &d)
        .await
        .unwrap();
    assert_eq!(p.stock_mil, 1500);
}
