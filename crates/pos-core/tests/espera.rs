mod comun;

use pos_core::servicios::espera::{self, NuevaEnEspera, MAXIMO_EN_ESPERA};
use pos_core::servicios::productos;
use pos_core::servicios::ventas::{self, ItemPedido, MedioPago, NuevaVenta};
use pos_core::Error;
use sqlx::SqlitePool;

fn carrito(nombre: Option<&str>, items: &[(i64, i64)]) -> NuevaEnEspera {
    NuevaEnEspera {
        nombre: nombre.map(str::to_owned),
        items: items
            .iter()
            .map(|&(producto_id, cantidad_mil)| ItemPedido {
                producto_id,
                cantidad_mil,
            })
            .collect(),
    }
}

async fn contar(pool: &SqlitePool, tabla: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {tabla}"))
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn pone_en_espera_y_lista_con_total_estimado() {
    let pool = comun::con_caja().await;
    let pan = comun::crear_kg(&pool, "P", "Pan", 2300, 10_000).await;
    let bebida = comun::crear(&pool, "B", "Bebida", 1500, 10).await;

    let id = espera::poner(
        &pool,
        &carrito(
            Some("  Señora del pan "),
            &[(pan.id, 532), (bebida.id, 2000)],
        ),
    )
    .await
    .unwrap();
    let sin_nombre = espera::poner(&pool, &carrito(Some("   "), &[(bebida.id, 1000)]))
        .await
        .unwrap();

    let lista = espera::listar(&pool).await.unwrap();
    assert_eq!(lista.len(), 2);
    assert_eq!(lista[0].id, id);
    assert_eq!(lista[0].nombre.as_deref(), Some("Señora del pan"));
    assert_eq!(lista[0].lineas, 2);
    // 532 g a $2.300/kg = $1.224, más 2 × $1.500.
    assert_eq!(lista[0].total_estimado, 1224 + 3000);
    assert_eq!(
        (lista[1].id, lista[1].nombre.as_deref()),
        (sin_nombre, None)
    );
    assert_eq!(lista[1].total_estimado, 1500);

    // El total usa los precios actuales.
    let mut d = comun::datos(Some("B"), "Bebida", 1700, 0);
    d.stock_inicial_mil = 0;
    productos::editar(&pool, bebida.id, &d).await.unwrap();
    assert_eq!(espera::listar(&pool).await.unwrap()[1].total_estimado, 1700);
}

#[tokio::test]
async fn recuperar_devuelve_las_lineas_y_borra_la_espera() {
    let pool = comun::con_caja().await;
    let pan = comun::crear_kg(&pool, "P", "Pan", 2300, 0).await;
    let bebida = comun::crear(&pool, "B", "Bebida", 1500, 0).await;
    let id = espera::poner(&pool, &carrito(None, &[(bebida.id, 3000), (pan.id, 750)]))
        .await
        .unwrap();

    let r = espera::recuperar(&pool, id, None).await.unwrap();
    let lineas: Vec<(String, i64)> = r
        .lineas
        .iter()
        .map(|l| (l.producto.nombre.clone(), l.cantidad_mil))
        .collect();
    assert_eq!(lineas, [("Bebida".into(), 3000), ("Pan".into(), 750)]);
    assert_eq!((r.saltados, r.intercambio_id), (0, None));

    assert!(espera::listar(&pool).await.unwrap().is_empty());
    assert_eq!(
        contar(&pool, "ventas_espera_items").await,
        0,
        "las líneas se borran en cascada"
    );

    let otra_vez = espera::recuperar(&pool, id, None).await;
    assert!(
        matches!(otra_vez, Err(Error::NoEncontrado(_))),
        "{otra_vez:?}"
    );
}

#[tokio::test]
async fn producto_desactivado_se_salta_al_recuperar() {
    let pool = comun::con_caja().await;
    let a = comun::crear(&pool, "A", "Activo", 1000, 0).await;
    let b = comun::crear(&pool, "B", "Se desactiva", 2000, 0).await;
    let id = espera::poner(&pool, &carrito(None, &[(a.id, 1000), (b.id, 1000)]))
        .await
        .unwrap();
    productos::cambiar_activo(&pool, b.id, false).await.unwrap();

    // La lista sigue contando la línea, pero el total solo suma productos activos.
    let resumen = &espera::listar(&pool).await.unwrap()[0];
    assert_eq!((resumen.lineas, resumen.total_estimado), (2, 1000));

    let r = espera::recuperar(&pool, id, None).await.unwrap();
    assert_eq!(r.lineas.len(), 1);
    assert_eq!(r.lineas[0].producto.id, a.id);
    assert_eq!(r.saltados, 1);
}

#[tokio::test]
async fn maximo_veinte_en_espera_e_intercambio_en_el_limite() {
    let pool = comun::con_caja().await;
    let p = comun::crear(&pool, "A", "Pan", 150, 0).await;
    let mut ids = Vec::new();
    for _ in 0..MAXIMO_EN_ESPERA {
        ids.push(
            espera::poner(&pool, &carrito(None, &[(p.id, 1000)]))
                .await
                .unwrap(),
        );
    }
    let otra = espera::poner(&pool, &carrito(None, &[(p.id, 1000)])).await;
    assert!(matches!(otra, Err(Error::Validacion(_))), "{otra:?}");

    // Intercambiar con 20 en espera sí se puede: la recuperada libera su lugar.
    let r = espera::recuperar(
        &pool,
        ids[0],
        Some(&carrito(Some("Actual"), &[(p.id, 2000)])),
    )
    .await
    .unwrap();
    let nueva = r.intercambio_id.expect("el carrito actual quedó en espera");
    let lista = espera::listar(&pool).await.unwrap();
    assert_eq!(lista.len() as i64, MAXIMO_EN_ESPERA);
    assert!(lista
        .iter()
        .any(|e| e.id == nueva && e.nombre.as_deref() == Some("Actual")));
}

#[tokio::test]
async fn no_cambia_stock_ni_folios() {
    let pool = comun::con_caja().await;
    let p = comun::crear(&pool, "A", "Leche", 1190, 10).await;

    let id = espera::poner(&pool, &carrito(None, &[(p.id, 4000)]))
        .await
        .unwrap();
    let r = espera::recuperar(&pool, id, None).await.unwrap();
    assert_eq!(r.lineas[0].producto.stock_mil, 10_000);
    let id = espera::poner(&pool, &carrito(None, &[(p.id, 1000)]))
        .await
        .unwrap();
    espera::descartar(&pool, id).await.unwrap();

    assert_eq!(contar(&pool, "ventas").await, 0);
    let movimientos_venta: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM movimientos_stock WHERE tipo = 'venta'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(movimientos_venta, 0);
    let stock: i64 = sqlx::query_scalar("SELECT stock_mil FROM productos WHERE id = ?")
        .bind(p.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stock, 10_000);

    // La primera venta cobrada sigue llevando el folio 1.
    let v = ventas::confirmar(
        &pool,
        &NuevaVenta {
            items: r
                .lineas
                .iter()
                .map(|l| ItemPedido {
                    producto_id: l.producto.id,
                    cantidad_mil: l.cantidad_mil,
                })
                .collect(),
            medio_pago: MedioPago::Debito,
            monto_recibido: None,
            descuento: 0,
            cliente_id: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(v.folio, 1);
}

#[tokio::test]
async fn rechazos_y_descartar() {
    let pool = comun::con_caja().await;
    let p = comun::crear(&pool, "A", "Pan", 150, 0).await;

    let vacio = espera::poner(&pool, &carrito(None, &[])).await;
    assert!(matches!(vacio, Err(Error::Validacion(_))), "{vacio:?}");
    let cero = espera::poner(&pool, &carrito(None, &[(p.id, 0)])).await;
    assert!(matches!(cero, Err(Error::Validacion(_))), "{cero:?}");
    let largo = "x".repeat(61);
    let nombre_largo = espera::poner(&pool, &carrito(Some(&largo), &[(p.id, 1000)])).await;
    assert!(
        matches!(nombre_largo, Err(Error::Validacion(_))),
        "{nombre_largo:?}"
    );
    let inexistente = espera::poner(&pool, &carrito(None, &[(999, 1000)])).await;
    assert!(
        matches!(inexistente, Err(Error::NoEncontrado(_))),
        "{inexistente:?}"
    );
    assert_eq!(
        contar(&pool, "ventas_espera").await,
        0,
        "los rechazos no guardan nada"
    );

    let id = espera::poner(&pool, &carrito(None, &[(p.id, 1000)]))
        .await
        .unwrap();
    espera::descartar(&pool, id).await.unwrap();
    assert!(espera::listar(&pool).await.unwrap().is_empty());
    assert!(matches!(
        espera::descartar(&pool, id).await,
        Err(Error::NoEncontrado(_))
    ));
}
