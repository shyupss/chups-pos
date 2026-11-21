// Historial de ventas: filtros, paginación, totales del filtro y detalle.

mod comun;

use chrono::{Duration, Local, NaiveTime, TimeZone, Utc};
use pos_core::servicios::historial::{self, EstadoFiltro, EstadoVenta, FiltrosVentas};
use pos_core::servicios::ventas::{self, ItemPedido, MedioPago, NuevaVenta, VentaConfirmada};
use sqlx::SqlitePool;

async fn vender(
    pool: &SqlitePool,
    producto_id: i64,
    unidades: i64,
    medio: MedioPago,
) -> VentaConfirmada {
    let cliente_id = if medio == MedioPago::Fiado {
        Some(
            sqlx::query_scalar("INSERT INTO clientes (nombre) VALUES ('Rosa') RETURNING id")
                .fetch_one(pool)
                .await
                .unwrap(),
        )
    } else {
        None
    };
    ventas::confirmar(
        pool,
        &NuevaVenta {
            items: vec![ItemPedido {
                producto_id,
                cantidad_mil: unidades * 1000,
            }],
            medio_pago: medio,
            monto_recibido: None,
            descuento: 0,
            cliente_id,
        },
    )
    .await
    .unwrap()
}

/// Día local a `dias` de hoy, como "aaaa-mm-dd".
fn dia(dias: i64) -> String {
    (Local::now().date_naive() + Duration::days(dias))
        .format("%Y-%m-%d")
        .to_string()
}

/// Mueve la venta al mediodía local de ese día (la base guarda UTC).
async fn fechar(pool: &SqlitePool, venta_id: i64, dias: i64) {
    let mediodia = (Local::now().date_naive() + Duration::days(dias))
        .and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap());
    let utc = Local
        .from_local_datetime(&mediodia)
        .earliest()
        .unwrap()
        .with_timezone(&Utc)
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    sqlx::query("UPDATE ventas SET fecha_hora = ? WHERE id = ?")
        .bind(utc)
        .bind(venta_id)
        .execute(pool)
        .await
        .unwrap();
}

async fn folios(pool: &SqlitePool, f: FiltrosVentas) -> Vec<i64> {
    historial::listar(pool, &f, 100, 0)
        .await
        .unwrap()
        .ventas
        .iter()
        .map(|v| v.folio)
        .collect()
}

#[tokio::test]
async fn filtra_por_fecha_folio_medio_y_estado() {
    let pool = comun::con_caja().await;
    let p = comun::crear(&pool, "B", "Bebida", 1000, 1000).await;
    // Folios 1 a 5: hoy, ayer, hace 3 días, hace 40 días, hoy.
    let a = vender(&pool, p.id, 1, MedioPago::Efectivo).await;
    let b = vender(&pool, p.id, 2, MedioPago::Debito).await;
    let c = vender(&pool, p.id, 3, MedioPago::Transferencia).await;
    let d = vender(&pool, p.id, 4, MedioPago::Efectivo).await;
    let e = vender(&pool, p.id, 5, MedioPago::Fiado).await;
    for (v, dias) in [(&a, 0), (&b, -1), (&c, -3), (&d, -40), (&e, 0)] {
        fechar(&pool, v.id, dias).await;
    }

    let rango = |desde: i64, hasta: i64| FiltrosVentas {
        desde: Some(dia(desde)),
        hasta: Some(dia(hasta)),
        ..Default::default()
    };
    // Hoy, de la más nueva a la más antigua (mismo mediodía: desempata el id).
    assert_eq!(folios(&pool, rango(0, 0)).await, [5, 1]);
    assert_eq!(folios(&pool, rango(-1, -1)).await, [2]);
    assert_eq!(folios(&pool, rango(-6, 0)).await, [5, 1, 2, 3]);
    assert_eq!(
        folios(&pool, FiltrosVentas::default()).await,
        [5, 1, 2, 3, 4]
    );

    // El folio manda sobre las fechas.
    let por_folio = FiltrosVentas {
        folio: Some(4),
        ..rango(0, 0)
    };
    assert_eq!(folios(&pool, por_folio).await, [4]);
    assert_eq!(historial::por_folio(&pool, 4).await.unwrap(), Some(d.id));
    assert_eq!(historial::por_folio(&pool, 99).await.unwrap(), None);

    let efectivo = FiltrosVentas {
        medio_pago: Some(MedioPago::Efectivo),
        ..Default::default()
    };
    assert_eq!(folios(&pool, efectivo).await, [1, 4]);

    // Estados: una anulada y una con devolución parcial (los servicios llegan en las partes 4 y 5).
    sqlx::query("UPDATE ventas SET estado = 'anulada', anulada_en = 'x' WHERE id = ?")
        .bind(b.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE ventas SET devolucion = 'parcial', monto_devuelto = 1000 WHERE id = ?")
        .bind(c.id)
        .execute(&pool)
        .await
        .unwrap();
    let estado = |e| FiltrosVentas {
        estado: Some(e),
        ..Default::default()
    };
    assert_eq!(folios(&pool, estado(EstadoFiltro::Pagada)).await, [1, 4]);
    assert_eq!(folios(&pool, estado(EstadoFiltro::Fiada)).await, [5]);
    assert_eq!(folios(&pool, estado(EstadoFiltro::Anulada)).await, [2]);
    assert_eq!(
        folios(&pool, estado(EstadoFiltro::ConDevolucion)).await,
        [3]
    );

    let lista = historial::listar(&pool, &FiltrosVentas::default(), 100, 0)
        .await
        .unwrap();
    let estados: Vec<(i64, EstadoVenta, i64)> = lista
        .ventas
        .iter()
        .map(|v| (v.folio, v.estado, v.productos))
        .collect();
    assert_eq!(
        estados,
        [
            (5, EstadoVenta::Fiada, 5),
            (1, EstadoVenta::Pagada, 1),
            (2, EstadoVenta::Anulada, 2),
            (3, EstadoVenta::ConDevolucion, 3),
            (4, EstadoVenta::Pagada, 4),
        ]
    );
    // Total del filtro: sin la anulada (2.000) y descontando lo devuelto (1.000).
    let t = lista.totales.unwrap();
    assert_eq!(t.cantidad, 5);
    assert_eq!(t.total, 1000 + 3000 - 1000 + 4000 + 5000);

    // Con un turno que no tiene ventas, nada.
    let otro_turno = FiltrosVentas {
        turno_id: Some(999),
        ..Default::default()
    };
    assert!(folios(&pool, otro_turno).await.is_empty());
}

#[tokio::test]
async fn pagina_sin_repetir_y_totaliza_en_la_primera_pagina() {
    let pool = comun::con_caja().await;
    let p = comun::crear(&pool, "B", "Bebida", 990, 1000).await;
    for _ in 0..25 {
        vender(&pool, p.id, 1, MedioPago::Debito).await;
    }
    let f = FiltrosVentas::default();
    let mut vistos = Vec::new();
    for (desde, esperadas) in [(0, 10), (10, 10), (20, 5), (30, 0)] {
        let pagina = historial::listar(&pool, &f, 10, desde).await.unwrap();
        assert_eq!(pagina.ventas.len(), esperadas);
        assert_eq!(pagina.totales.is_some(), desde == 0);
        if desde == 0 {
            let t = pagina.totales.unwrap();
            assert_eq!((t.cantidad, t.total), (25, 25 * 990));
        }
        vistos.extend(pagina.ventas.iter().map(|v| v.folio));
    }
    assert_eq!(vistos, (1..=25).rev().collect::<Vec<i64>>());
}

#[tokio::test]
async fn filtra_por_turno() {
    let pool = comun::con_caja().await;
    let control = pos_core::servicios::seguridad::ControlPin::new();
    let p = comun::crear(&pool, "B", "Bebida", 1000, 1000).await;
    let primero = pos_core::servicios::turnos::estado_caja(&pool)
        .await
        .unwrap()
        .turno
        .unwrap();
    vender(&pool, p.id, 1, MedioPago::Efectivo).await;
    pos_core::servicios::turnos::cerrar(&pool, &control, 1000, None, None, None)
        .await
        .unwrap();
    let segundo = pos_core::servicios::turnos::abrir(&pool, 0).await.unwrap();
    vender(&pool, p.id, 1, MedioPago::Efectivo).await;
    vender(&pool, p.id, 1, MedioPago::Efectivo).await;

    let de = |turno_id| FiltrosVentas {
        turno_id: Some(turno_id),
        ..Default::default()
    };
    assert_eq!(folios(&pool, de(primero.id)).await, [1]);
    assert_eq!(folios(&pool, de(segundo.id)).await, [3, 2]);
}

#[tokio::test]
async fn detalle_con_lineas_pago_y_turno() {
    let pool = comun::con_caja().await;
    let pan = comun::crear_kg(&pool, "P", "Pan", 2300, 10_000).await;
    let bebida = comun::crear(&pool, "B", "Bebida", 1500, 10).await;
    let v = ventas::confirmar(
        &pool,
        &NuevaVenta {
            items: vec![
                ItemPedido {
                    producto_id: pan.id,
                    cantidad_mil: 532,
                },
                ItemPedido {
                    producto_id: bebida.id,
                    cantidad_mil: 2000,
                },
            ],
            medio_pago: MedioPago::Efectivo,
            monto_recibido: Some(5000),
            descuento: 0,
            cliente_id: None,
        },
    )
    .await
    .unwrap();

    let d = historial::detalle(&pool, v.id).await.unwrap();
    assert_eq!((d.folio, d.total, d.redondeo), (v.folio, 4220, -4));
    assert_eq!((d.monto_recibido, d.vuelto), (Some(5000), Some(780)));
    assert_eq!(d.estado, EstadoVenta::Pagada);
    assert!(d.turno_id.is_some());
    assert!(d.anulacion.is_none() && d.devoluciones.is_empty());
    assert_eq!(d.lineas.len(), 2);
    assert_eq!(
        (
            d.lineas[0].nombre.as_str(),
            d.lineas[0].cantidad_mil,
            d.lineas[0].devuelto_mil
        ),
        ("Pan", 532, 0)
    );
    assert!(historial::detalle(&pool, 999).await.is_err());

    let lista = historial::listar(&pool, &FiltrosVentas::default(), 10, 0)
        .await
        .unwrap();
    // 0,532 kg de pan cuenta como 1 y las 2 bebidas como 2.
    assert_eq!(lista.ventas[0].productos, 3);
}

#[tokio::test]
async fn rechaza_fechas_invalidas() {
    let pool = comun::con_caja().await;
    let f = FiltrosVentas {
        desde: Some(dia(0)),
        hasta: Some(dia(-1)),
        ..Default::default()
    };
    assert!(historial::listar(&pool, &f, 10, 0).await.is_err());
}
