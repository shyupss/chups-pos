// Inventario: stock filtrado, valorización, ajuste por conteo, mermas y movimientos.

mod comun;

use std::time::{Duration, Instant};

use pos_core::servicios::inventario::{
    self, Contado, CursorMovimientos, FiltroEstadoStock, FiltrosMovimientos, FiltrosStock,
    MotivoMerma, TipoMovimientoStock,
};
use pos_core::servicios::productos;
use pos_core::servicios::seguridad::{self, ControlPin};
use pos_core::{db, Error, FalloPin};
use sqlx::SqlitePool;

async fn stock(pool: &SqlitePool, codigo: &str) -> i64 {
    productos::buscar_por_codigo(pool, codigo)
        .await
        .unwrap()
        .unwrap()
        .stock_mil
}

async fn ajustar(
    pool: &SqlitePool,
    items: &[(i64, i64)],
) -> Result<Vec<inventario::Diferencia>, Error> {
    let items: Vec<Contado> = items
        .iter()
        .map(|&(producto_id, contado_mil)| Contado {
            producto_id,
            contado_mil,
        })
        .collect();
    inventario::ajustar_conteo(pool, &ControlPin::new(), &items, "Conteo mensual", None).await
}

#[tokio::test]
async fn ajuste_por_conteo_positivo_y_negativo_por_unidad_y_kg() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let leche = comun::crear(&pool, "L", "Leche", 1190, 10).await;
    let queso = comun::crear_kg(&pool, "Q", "Queso", 10_000, 2_500).await;
    sqlx::query("UPDATE productos SET costo = 800 WHERE id = ?")
        .bind(leche.id)
        .execute(&pool)
        .await
        .unwrap();

    // Vista previa: no cambia nada.
    let vista = inventario::previsualizar_conteo(
        &pool,
        &[
            Contado {
                producto_id: leche.id,
                contado_mil: 12_000,
            },
            Contado {
                producto_id: queso.id,
                contado_mil: 2_318,
            },
        ],
    )
    .await
    .unwrap();
    assert_eq!(vista[0].diferencia_mil, 2000);
    assert_eq!(vista[0].valor, Some(1600));
    assert_eq!((vista[1].diferencia_mil, vista[1].valor), (-182, None));
    assert_eq!(stock(&pool, "L").await, 10_000);

    let d = ajustar(&pool, &[(leche.id, 12_000), (queso.id, 2_318)])
        .await
        .unwrap();
    assert_eq!(d.len(), 2);
    assert_eq!(
        (stock(&pool, "L").await, stock(&pool, "Q").await),
        (12_000, 2_318)
    );

    // Movimientos 'ajuste' con la diferencia y el motivo.
    let movs: Vec<(String, i64, Option<String>)> = sqlx::query_as(
        "SELECT tipo, cantidad_mil, motivo FROM movimientos_stock
         WHERE nota = 'Conteo' ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        movs,
        [
            ("ajuste".to_owned(), 2000, Some("Conteo mensual".to_owned())),
            ("ajuste".to_owned(), -182, Some("Conteo mensual".to_owned())),
        ]
    );

    // Sin diferencia no hay movimiento.
    ajustar(&pool, &[(leche.id, 12_000)]).await.unwrap();
    let total: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM movimientos_stock WHERE nota = 'Conteo'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(total, 2);
}

#[tokio::test]
async fn el_conteo_multiple_es_una_sola_transaccion() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let leche = comun::crear(&pool, "L", "Leche", 1190, 10).await;
    let pan = comun::crear(&pool, "P", "Pan", 250, 50).await;

    // La segunda línea es inválida (media unidad): no se aplica ninguna.
    assert!(matches!(
        ajustar(&pool, &[(leche.id, 4000), (pan.id, 1500)]).await,
        Err(Error::Validacion(m)) if m.contains("número entero")
    ));
    assert!(ajustar(&pool, &[(leche.id, 4000), (leche.id, 3000)])
        .await
        .is_err());
    assert!(ajustar(&pool, &[(leche.id, -1000)]).await.is_err());
    assert!(ajustar(&pool, &[]).await.is_err());
    assert!(inventario::ajustar_conteo(
        &pool,
        &ControlPin::new(),
        &[Contado {
            producto_id: leche.id,
            contado_mil: 1000
        }],
        "  ",
        None
    )
    .await
    .is_err());
    assert_eq!(
        (stock(&pool, "L").await, stock(&pool, "P").await),
        (10_000, 50_000)
    );

    ajustar(&pool, &[(leche.id, 0), (pan.id, 48_000)])
        .await
        .unwrap();
    assert_eq!(
        (stock(&pool, "L").await, stock(&pool, "P").await),
        (0, 48_000)
    );
}

#[tokio::test]
async fn ajustes_y_mermas_piden_pin_cuando_esta_protegido() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let leche = comun::crear(&pool, "L", "Leche", 1190, 10).await;
    seguridad::crear_pin(&pool, "4821").await.unwrap();
    let control = ControlPin::new();
    let items = [Contado {
        producto_id: leche.id,
        contado_mil: 7000,
    }];

    assert!(matches!(
        inventario::ajustar_conteo(&pool, &control, &items, "Conteo", None).await,
        Err(Error::Pin(FalloPin::Requerido))
    ));
    assert!(matches!(
        inventario::registrar_merma(
            &pool,
            &control,
            leche.id,
            1000,
            MotivoMerma::Vencido,
            None,
            None
        )
        .await,
        Err(Error::Pin(FalloPin::Requerido))
    ));
    assert_eq!(stock(&pool, "L").await, 10_000);
    inventario::ajustar_conteo(&pool, &control, &items, "Conteo", Some("4821"))
        .await
        .unwrap();
    assert_eq!(stock(&pool, "L").await, 7000);

    // Si se desprotege, ya no lo pide.
    let mut p = seguridad::estado(&pool).await.unwrap().protegidas;
    p.ajustar_inventario = false;
    seguridad::guardar_protegidas(&pool, &control, p, Some("4821"))
        .await
        .unwrap();
    inventario::registrar_merma(
        &pool,
        &control,
        leche.id,
        1000,
        MotivoMerma::Danado,
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(stock(&pool, "L").await, 6000);
}

#[tokio::test]
async fn merma_con_motivo_por_unidad_y_por_kg() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let leche = comun::crear(&pool, "L", "Leche", 1190, 10).await;
    let queso = comun::crear_kg(&pool, "Q", "Queso", 10_000, 2_500).await;
    let control = ControlPin::new();

    let quedan = inventario::registrar_merma(
        &pool,
        &control,
        queso.id,
        318,
        MotivoMerma::Vencido,
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(quedan, 2_182);
    inventario::registrar_merma(
        &pool,
        &control,
        leche.id,
        2000,
        MotivoMerma::Otro,
        Some(" Se cayó la caja "),
        None,
    )
    .await
    .unwrap();
    assert!(inventario::registrar_merma(
        &pool,
        &control,
        leche.id,
        1000,
        MotivoMerma::Otro,
        Some("  "),
        None
    )
    .await
    .is_err());
    assert!(inventario::registrar_merma(
        &pool,
        &control,
        leche.id,
        500,
        MotivoMerma::Robo,
        None,
        None
    )
    .await
    .is_err());
    assert!(inventario::registrar_merma(
        &pool,
        &control,
        leche.id,
        0,
        MotivoMerma::Robo,
        None,
        None
    )
    .await
    .is_err());

    let pagina = inventario::listar_movimientos(
        &pool,
        &FiltrosMovimientos {
            tipo: Some(TipoMovimientoStock::Merma),
            ..Default::default()
        },
        None,
        50,
    )
    .await
    .unwrap();
    let mermas: Vec<(String, i64, Option<String>)> = pagina
        .movimientos
        .iter()
        .map(|m| (m.producto.clone(), m.cantidad_mil, m.motivo.clone()))
        .collect();
    assert_eq!(
        mermas,
        [
            (
                "Leche".to_owned(),
                -2000,
                Some("Otro: Se cayó la caja".to_owned())
            ),
            ("Queso".to_owned(), -318, Some("Vencido".to_owned())),
        ]
    );
}

#[tokio::test]
async fn valoriza_el_inventario_y_filtra_el_stock() {
    let pool = db::abrir_en_memoria().await.unwrap();
    // Leche: 10 × $800 = $8.000 (venta 10 × $1.190). Queso: 2,5 kg × $7.000 = $17.500.
    // Pan: sin costo (cuenta 0 y se informa). Bebida: stock negativo (no suma).
    let leche = comun::crear(&pool, "L", "Leche", 1190, 10).await;
    let queso = comun::crear_kg(&pool, "Q", "Queso", 10_000, 2_500).await;
    let pan = comun::crear(&pool, "P", "Pan", 250, 4).await;
    let bebida = comun::crear(&pool, "B", "Bebida", 1500, 0).await;
    let agua = comun::crear(&pool, "A", "Agua", 700, 0).await;
    for (sql, id) in [
        (
            "UPDATE productos SET costo = 800, stock_minimo_mil = 12000 WHERE id = ?",
            leche.id,
        ),
        ("UPDATE productos SET costo = 7000 WHERE id = ?", queso.id),
        (
            "UPDATE productos SET stock_minimo_mil = 2000 WHERE id = ?",
            pan.id,
        ),
        (
            "UPDATE productos SET stock_mil = -3000, costo = 900 WHERE id = ?",
            bebida.id,
        ),
    ] {
        sqlx::query(sql).bind(id).execute(&pool).await.unwrap();
    }

    let r = inventario::resumen(&pool).await.unwrap();
    assert_eq!(r.valor_costo, 8000 + 17_500);
    assert_eq!(r.valor_venta, 11_900 + 25_000 + 1000);
    assert_eq!(r.sin_costo, 1);
    assert_eq!(r.bajo_minimo, 1);
    assert_eq!(r.productos, 5);
    assert_eq!(inventario::contar_bajo_minimo(&pool).await.unwrap(), 1);

    let nombres = |estado| {
        let pool = &pool;
        async move {
            inventario::listar_stock(
                pool,
                &FiltrosStock {
                    estado,
                    ..Default::default()
                },
                100,
                0,
            )
            .await
            .unwrap()
            .into_iter()
            .map(|p| p.nombre)
            .collect::<Vec<_>>()
        }
    };
    assert_eq!(
        nombres(FiltroEstadoStock::Todos).await,
        ["Agua", "Bebida", "Leche", "Pan", "Queso"]
    );
    assert_eq!(nombres(FiltroEstadoStock::BajoMinimo).await, ["Leche"]);
    assert_eq!(nombres(FiltroEstadoStock::SinStock).await, ["Agua"]);
    assert_eq!(nombres(FiltroEstadoStock::Negativo).await, ["Bebida"]);

    let lista = inventario::listar_stock(&pool, &FiltrosStock::default(), 100, 0)
        .await
        .unwrap();
    let valor = |n: &str| lista.iter().find(|p| p.nombre == n).unwrap().valor;
    assert_eq!(valor("Leche"), Some(8000));
    assert_eq!(valor("Pan"), None);
    assert_eq!(valor("Bebida"), Some(0));
    assert!(
        lista
            .iter()
            .find(|p| p.nombre == "Leche")
            .unwrap()
            .bajo_minimo
    );

    let buscados = inventario::listar_stock(
        &pool,
        &FiltrosStock {
            texto: "que".into(),
            ..Default::default()
        },
        100,
        0,
    )
    .await
    .unwrap();
    assert_eq!(buscados.len(), 1);

    // Desactivar un producto lo saca de la cuenta.
    productos::cambiar_activo(&pool, leche.id, false)
        .await
        .unwrap();
    assert_eq!(inventario::contar_bajo_minimo(&pool).await.unwrap(), 0);
    let _ = agua;
}

#[tokio::test]
async fn movimientos_filtrados_y_paginados_con_cursor() {
    let pool = comun::con_caja().await;
    let leche = comun::crear(&pool, "L", "Leche", 1190, 10).await;
    let pan = comun::crear(&pool, "P", "Pan", 250, 10).await;
    let v = pos_core::servicios::ventas::confirmar(
        &pool,
        &pos_core::servicios::ventas::NuevaVenta {
            items: vec![pos_core::servicios::ventas::ItemPedido {
                producto_id: leche.id,
                cantidad_mil: 2000,
            }],
            medio_pago: pos_core::servicios::ventas::MedioPago::Debito,
            monto_recibido: None,
            descuento: 0,
            cliente_id: None,
        },
    )
    .await
    .unwrap();
    ajustar(&pool, &[(leche.id, 9000)]).await.unwrap();

    let de_leche = inventario::listar_movimientos(
        &pool,
        &FiltrosMovimientos {
            producto_id: Some(leche.id),
            ..Default::default()
        },
        None,
        50,
    )
    .await
    .unwrap();
    // Del más nuevo al más antiguo: ajuste +1, venta −2, stock inicial +10.
    let resumen: Vec<(TipoMovimientoStock, i64, i64)> = de_leche
        .movimientos
        .iter()
        .map(|m| (m.tipo, m.cantidad_mil, m.stock_resultante_mil))
        .collect();
    assert_eq!(
        resumen,
        [
            (TipoMovimientoStock::Ajuste, 1000, 9000),
            (TipoMovimientoStock::Venta, -2000, 8000),
            (TipoMovimientoStock::Ajuste, 10_000, 10_000),
        ]
    );
    assert_eq!(de_leche.movimientos[1].folio, Some(v.folio));
    assert!(de_leche.siguiente.is_none());

    // De a uno con el cursor: los cuatro movimientos, sin repetir.
    let mut vistos = Vec::new();
    let mut cursor: Option<CursorMovimientos> = None;
    loop {
        let p = inventario::listar_movimientos(&pool, &FiltrosMovimientos::default(), cursor, 1)
            .await
            .unwrap();
        vistos.extend(p.movimientos.iter().map(|m| m.id));
        match p.siguiente {
            Some(c) => cursor = Some(c),
            None => break,
        }
    }
    vistos.dedup();
    assert_eq!(vistos.len(), 4);
    let _ = pan;
}

/// 100.000 movimientos: la primera página y una página profunda (por cursor) responden en pocos
/// milisegundos, porque nunca se ordena en memoria ni se salta con OFFSET.
#[tokio::test]
async fn la_lista_de_movimientos_es_fluida_con_cien_mil_filas() {
    let dir = tempfile::tempdir().unwrap();
    let pool = db::abrir(&dir.path().join("grande.db")).await.unwrap();
    for i in 0..20 {
        comun::crear(&pool, &format!("C{i}"), &format!("Producto {i}"), 1000, 0).await;
    }
    sqlx::query(
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 100000)
         INSERT INTO movimientos_stock (producto_id, tipo, cantidad_mil, stock_resultante_mil, fecha)
         SELECT 1 + i % 20, CASE i % 3 WHEN 0 THEN 'venta' WHEN 1 THEN 'compra' ELSE 'ajuste' END,
                1000, i, strftime('%Y-%m-%dT%H:%M:%fZ', '2026-01-01', '+' || i || ' minutes')
         FROM n",
    )
    .execute(&pool)
    .await
    .unwrap();

    let medir = |f: FiltrosMovimientos, paginas: usize| {
        let pool = &pool;
        async move {
            let mut cursor = None;
            let mut peor = Duration::ZERO;
            let mut filas = 0;
            for _ in 0..paginas {
                let inicio = Instant::now();
                let p = inventario::listar_movimientos(pool, &f, cursor, 200)
                    .await
                    .unwrap();
                peor = peor.max(inicio.elapsed());
                filas += p.movimientos.len();
                cursor = p.siguiente;
            }
            (peor, filas)
        }
    };
    // 300 páginas de 200 = 60.000 filas recorridas, con y sin filtros.
    let (todas, filas) = medir(FiltrosMovimientos::default(), 300).await;
    assert_eq!(filas, 60_000);
    let (por_tipo, _) = medir(
        FiltrosMovimientos {
            tipo: Some(TipoMovimientoStock::Compra),
            ..Default::default()
        },
        100,
    )
    .await;
    let (por_producto, _) = medir(
        FiltrosMovimientos {
            producto_id: Some(3),
            ..Default::default()
        },
        20,
    )
    .await;
    println!("peor página: todas {todas:?}, por tipo {por_tipo:?}, por producto {por_producto:?}");
    // Holgado para un build de pruebas sin optimizar en una CI lenta.
    for peor in [todas, por_tipo, por_producto] {
        assert!(
            peor < Duration::from_millis(250),
            "una página tardó {peor:?}"
        );
    }
}
