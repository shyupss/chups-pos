// Reportes: totales que cuadran con el historial de ventas, anuladas fuera, devoluciones
// descontadas, días locales cruzando los cambios de horario de Chile, productos por kg, ganancia
// sin inventar costos, turnos, fiados y la pantalla Hoy (también con 200.000 ventas).

mod comun;

use std::time::{Duration, Instant};

use chrono::{NaiveDate, TimeZone, Utc};
use pos_core::db;
use pos_core::servicios::anulacion;
use pos_core::servicios::clientes::{self, DatosCliente};
use pos_core::servicios::devoluciones::{self, ItemDevuelto};
use pos_core::servicios::fiados::{self, NuevoPagoCliente};
use pos_core::servicios::historial::{self, FiltrosVentas};
use pos_core::servicios::productos::{self, DatosProducto};
use pos_core::servicios::reportes::{self, Rango};
use pos_core::servicios::seguridad::ControlPin;
use pos_core::servicios::turnos;
use pos_core::servicios::ventas::{self, ItemPedido, MedioPago, NuevaVenta};
use pos_core::zona::{Zona, ZonaConCambio, ZonaLocal};
use sqlx::SqlitePool;

fn venta(items: &[(i64, i64)], medio: MedioPago, cliente: Option<i64>) -> NuevaVenta {
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
        cliente_id: cliente,
    }
}

async fn vender(pool: &SqlitePool, items: &[(i64, i64)], medio: MedioPago) -> i64 {
    ventas::confirmar(pool, &venta(items, medio, None))
        .await
        .unwrap()
        .id
}

async fn fechar(pool: &SqlitePool, venta_id: i64, fecha: &str) {
    sqlx::query("UPDATE ventas SET fecha_hora = ? WHERE id = ?")
        .bind(fecha)
        .bind(venta_id)
        .execute(pool)
        .await
        .unwrap();
}

async fn con_costo(
    pool: &SqlitePool,
    codigo: &str,
    nombre: &str,
    precio: i64,
    costo: Option<i64>,
) -> i64 {
    let mut d = comun::datos(Some(codigo), nombre, precio, 1000);
    d.costo = costo;
    productos::crear(pool, &d).await.unwrap().id
}

fn dia(a: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(a, m, d).unwrap()
}

fn hoy() -> Rango {
    let d = ZonaLocal.dia(Utc::now());
    Rango { desde: d, hasta: d }
}

#[tokio::test]
async fn los_totales_cuadran_con_el_historial_sin_anuladas_y_con_devoluciones() {
    let pool = comun::con_caja().await;
    let control = ControlPin::new();
    let a = con_costo(&pool, "A", "Leche", 1190, Some(850)).await;
    let b = con_costo(&pool, "B", "Pan", 990, None).await;
    // Débito para que no haya redondeos que distraigan.
    let v1 = vender(&pool, &[(a, 2000), (b, 1000)], MedioPago::Debito).await; // 3.370
    vender(&pool, &[(a, 1000)], MedioPago::Credito).await; // 1.190
    let v3 = vender(&pool, &[(b, 3000)], MedioPago::Transferencia).await; // 2.970
    anulacion::anular(
        &pool,
        &control,
        v3,
        "Error",
        Some(MedioPago::Transferencia),
        None,
    )
    .await
    .unwrap();
    let linea = historial::detalle(&pool, v1).await.unwrap().lineas[0].id;
    devoluciones::registrar(
        &pool,
        &control,
        v1,
        &[ItemDevuelto {
            venta_item_id: linea,
            cantidad_mil: 1000,
            vuelve_a_stock: true,
        }],
        Some(MedioPago::Debito),
        "Venía abierta",
        None,
    )
    .await
    .unwrap();

    let r = reportes::ventas(&pool, &ZonaLocal, hoy()).await.unwrap();
    assert_eq!((r.total, r.boletas), (3370 - 1190 + 1190, 2));
    let h = historial::listar(
        &pool,
        &FiltrosVentas {
            desde: Some(r.desde.clone()),
            hasta: Some(r.hasta.clone()),
            folio: None,
            medio_pago: None,
            estado: None,
            turno_id: None,
        },
        50,
        0,
    )
    .await
    .unwrap()
    .totales
    .unwrap();
    assert_eq!(h.total, r.total);
    assert_eq!(r.por_medio.iter().map(|m| m.monto).sum::<i64>(), r.total);
    assert_eq!(r.por_dia.len(), 1);
    assert_eq!(r.por_dia[0].monto, r.total);
    assert_eq!(r.ticket_promedio, (r.total + 1) / 2);

    // Productos: la leche vendió 3 y se devolvió 1; el pan de la anulada no cuenta.
    let p = reportes::productos_vendidos(&pool, &ZonaLocal, hoy(), None)
        .await
        .unwrap();
    let cant: Vec<(String, i64, i64)> = p
        .productos
        .iter()
        .map(|x| (x.nombre.clone(), x.cantidad_mil, x.monto))
        .collect();
    assert_eq!(
        cant,
        [("Leche".into(), 2000, 2380), ("Pan".into(), 1000, 990)]
    );
    assert!(p.sin_ventas.is_empty());
}

#[tokio::test]
async fn agrupa_por_dia_local_cruzando_los_cambios_de_horario() {
    let pool = comun::con_caja().await;
    let a = con_costo(&pool, "A", "Leche", 1000, Some(600)).await;
    let mut ids = Vec::new();
    for _ in 0..6 {
        ids.push(vender(&pool, &[(a, 1000)], MedioPago::Debito).await);
    }
    // Abril: el 4 dura 25 horas (las 23:xx se repiten).
    fechar(&pool, ids[0], "2026-04-04T02:59:00.000Z").await; // 3 de abril 23:59
    fechar(&pool, ids[1], "2026-04-05T02:30:00.000Z").await; // 4 de abril 23:30 (UTC−3)
    fechar(&pool, ids[2], "2026-04-05T03:30:00.000Z").await; // 4 de abril 23:30 (UTC−4)
    fechar(&pool, ids[3], "2026-04-05T04:30:00.000Z").await; // 5 de abril 00:30
    let abril = ZonaConCambio::chile_abril_2026();
    let r = reportes::ventas(
        &pool,
        &abril,
        Rango {
            desde: dia(2026, 4, 4),
            hasta: dia(2026, 4, 5),
        },
    )
    .await
    .unwrap();
    let dias: Vec<(String, i64)> = r
        .por_dia
        .iter()
        .map(|d| (d.dia.clone(), d.boletas))
        .collect();
    assert_eq!(dias, [("2026-04-04".into(), 2), ("2026-04-05".into(), 1)]);
    // Sábado 4 a las 23 (dos veces) y domingo 5 a las 0.
    assert_eq!(r.mapa[5][23].boletas, 2);
    assert_eq!(r.mapa[6][0].boletas, 1);
    assert_eq!(r.por_dia_semana[5].monto, 2000);

    // Septiembre: el 6 empieza a la 01:00 (no hay medianoche).
    fechar(&pool, ids[4], "2026-09-06T03:50:00.000Z").await; // 5 de sept. 23:50 (UTC−4)
    fechar(&pool, ids[5], "2026-09-06T04:10:00.000Z").await; // 6 de sept. 01:10 (UTC−3)
    let sept = ZonaConCambio::chile_septiembre_2026();
    let r = reportes::ventas(
        &pool,
        &sept,
        Rango {
            desde: dia(2026, 9, 6),
            hasta: dia(2026, 9, 6),
        },
    )
    .await
    .unwrap();
    assert_eq!((r.boletas, r.por_dia.len()), (1, 1));
    assert_eq!(r.mapa[6][1].boletas, 1);
    let r = reportes::ventas(
        &pool,
        &sept,
        Rango {
            desde: dia(2026, 9, 5),
            hasta: dia(2026, 9, 5),
        },
    )
    .await
    .unwrap();
    assert_eq!(r.boletas, 1);
    assert_eq!(r.mapa[5][23].boletas, 1);
}

#[tokio::test]
async fn productos_por_kg_y_ganancia_sin_costo_excluida() {
    let pool = comun::con_caja().await;
    let control = ControlPin::new();
    let mut d = comun::datos(Some("Q"), "Queso", 10_000, 0);
    d.unidad = pos_core::repositorios::productos::Unidad::Kg;
    d.stock_inicial_mil = 5000;
    d.costo = Some(7000);
    let queso = productos::crear(&pool, &d).await.unwrap().id;
    let leche = con_costo(&pool, "L", "Leche", 1000, Some(600)).await;
    let pan = con_costo(&pool, "P", "Pan", 500, None).await;

    // 0,532 kg de queso ($5.320, costo $3.724), 2 leches ($2.000, costo $1.200) y 3 panes sin costo.
    let v = vender(
        &pool,
        &[(queso, 532), (leche, 2000), (pan, 3000)],
        MedioPago::Debito,
    )
    .await;
    // Se devuelven 0,132 kg de queso: $1.320 y $924 de costo.
    let linea = historial::detalle(&pool, v).await.unwrap().lineas[0].id;
    devoluciones::registrar(
        &pool,
        &control,
        v,
        &[ItemDevuelto {
            venta_item_id: linea,
            cantidad_mil: 132,
            vuelve_a_stock: true,
        }],
        Some(MedioPago::Debito),
        "Sobró",
        None,
    )
    .await
    .unwrap();

    let p = reportes::productos_vendidos(&pool, &ZonaLocal, hoy(), None)
        .await
        .unwrap();
    let q = p.productos.iter().find(|x| x.producto_id == queso).unwrap();
    assert_eq!((q.cantidad_mil, q.monto), (400, 4000));
    assert_eq!(q.unidad, pos_core::repositorios::productos::Unidad::Kg);

    let g = reportes::ganancia(&pool, &ZonaLocal, hoy()).await.unwrap();
    assert_eq!((g.ventas, g.costo, g.ganancia), (6000, 2800 + 1200, 2000));
    assert_eq!(g.margen_pb, Some(3333));
    assert_eq!(g.ventas_sin_costo, 1500);
    assert_eq!(g.sin_costo.len(), 1);
    assert_eq!(g.sin_costo[0].nombre, "Pan");
    assert!(g.productos.iter().all(|x| x.nombre != "Pan"));
    assert_eq!(g.ventas_estimadas, 0);

    // Una línea antigua con costo estimado se informa aparte.
    sqlx::query("UPDATE venta_items SET costo_estimado = 1 WHERE producto_id = ?")
        .bind(leche)
        .execute(&pool)
        .await
        .unwrap();
    let g = reportes::ganancia(&pool, &ZonaLocal, hoy()).await.unwrap();
    assert_eq!((g.ventas_estimadas, g.costo_estimado), (2000, 1200));
    assert!(g
        .productos
        .iter()
        .any(|x| x.nombre == "Leche" && x.costo_estimado));
    let categorias: i64 = g.por_categoria.iter().map(|c| c.ganancia).sum();
    assert_eq!(categorias, g.ganancia);
}

#[tokio::test]
async fn turnos_cerrados_y_fiados_al_cierre_del_rango() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let control = ControlPin::new();
    let leche = con_costo(&pool, "L", "Leche", 1000, Some(600)).await;
    turnos::abrir(&pool, 10_000).await.unwrap();
    vender(&pool, &[(leche, 1000)], MedioPago::Efectivo).await;
    // Se cuentan $200 de menos.
    turnos::cerrar(&pool, &control, 10_800, None, None, None)
        .await
        .unwrap();
    turnos::abrir(&pool, 0).await.unwrap();

    let rosa = clientes::crear(
        &pool,
        &DatosCliente {
            nombre: "Rosa".into(),
            telefono: None,
            rut: None,
            direccion: None,
            nota: None,
            limite_credito: None,
        },
    )
    .await
    .unwrap()
    .id;
    let vieja = ventas::confirmar(
        &pool,
        &venta(&[(leche, 5000)], MedioPago::Fiado, Some(rosa)),
    )
    .await
    .unwrap()
    .id;
    ventas::confirmar(
        &pool,
        &venta(&[(leche, 2000)], MedioPago::Fiado, Some(rosa)),
    )
    .await
    .unwrap();
    let hace_45 = Utc::now() - chrono::Duration::days(45);
    fechar(&pool, vieja, &pos_core::zona::a_texto(hace_45)).await;
    fiados::registrar_pago(
        &pool,
        &NuevoPagoCliente {
            cliente_id: rosa,
            monto: 1500,
            medio_pago: MedioPago::Debito,
            ventas: vec![],
            monto_recibido: None,
            nota: None,
        },
    )
    .await
    .unwrap();

    let t = reportes::turnos(&pool, &ZonaLocal, hoy()).await.unwrap();
    assert_eq!(t.turnos.len(), 1);
    assert_eq!(
        (
            t.turnos[0].esperado,
            t.turnos[0].contado,
            t.turnos[0].diferencia
        ),
        (11_000, 10_800, -200)
    );
    assert_eq!((t.sobrantes, t.faltantes, t.cuadrados), (0, 200, 0));

    let f = reportes::fiados(&pool, &ZonaLocal, hoy(), Utc::now())
        .await
        .unwrap();
    // Hoy se fió $2.000 (la de hace 45 días no es de hoy) y se pagaron $1.500 (a la más antigua).
    assert_eq!((f.otorgado, f.pagado), (2000, 1500));
    assert_eq!((f.deuda_al_cierre, f.clientes_con_deuda), (5500, 1));
    let tramos: Vec<i64> = f.antiguedad.iter().map(|t| t.monto).collect();
    assert_eq!(tramos, [2000, 3500, 0, 0]);
    assert_eq!(f.deudores[0].nombre, "Rosa");

    // Al cierre de un rango que terminó hace 40 días, el pago y la venta nueva no existían.
    let hace_40 = ZonaLocal.dia(Utc::now() - chrono::Duration::days(40));
    let f = reportes::fiados(
        &pool,
        &ZonaLocal,
        Rango {
            desde: hace_40 - chrono::Duration::days(10),
            hasta: hace_40,
        },
        Utc::now(),
    )
    .await
    .unwrap();
    assert_eq!((f.deuda_al_cierre, f.otorgado, f.pagado), (5000, 5000, 0));
}

#[tokio::test]
async fn la_pantalla_hoy_resume_el_dia() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let mut d = comun::datos(Some("L"), "Leche", 1000, 5);
    d.costo = Some(600);
    d.stock_minimo_mil = 10_000;
    let leche = productos::crear(&pool, &d).await.unwrap().id;
    let pan = con_costo(&pool, "P", "Pan", 500, None).await;
    turnos::abrir(&pool, 5000).await.unwrap();
    vender(&pool, &[(leche, 2000), (pan, 1000)], MedioPago::Efectivo).await; // 2.500
    vender(&pool, &[(leche, 1000)], MedioPago::Debito).await; // 1.000
                                                              // Hace una semana, a esta misma hora, se había vendido $1.000.
    let antes = vender(&pool, &[(pan, 2000)], MedioPago::Debito).await;
    fechar(
        &pool,
        antes,
        &pos_core::zona::a_texto(
            Utc::now() - chrono::Duration::days(7) - chrono::Duration::minutes(5),
        ),
    )
    .await;

    let inicio = Instant::now();
    let h = reportes::hoy(&pool, &ZonaLocal, Utc::now()).await.unwrap();
    let tiempo = inicio.elapsed();
    assert_eq!((h.ventas, h.boletas, h.ticket_promedio), (3500, 2, 1750));
    assert_eq!((h.ganancia, h.ventas_sin_costo), (1200, 500));
    assert_eq!(
        (h.ventas_semana_anterior, h.variacion_pb),
        (1000, Some(25_000))
    );
    assert_eq!(h.efectivo_esperado, Some(7500));
    assert_eq!(h.por_hora.iter().map(|c| c.boletas).sum::<i64>(), 2);
    assert_eq!(h.por_medio.len(), 2);
    assert_eq!(h.mas_vendidos[0].nombre, "Leche");
    assert_eq!((h.total_bajo_minimo, h.bajo_minimo.len()), (1, 1));
    assert!(tiempo < Duration::from_millis(100), "{tiempo:?}");
}

/// 200.000 ventas en un año (con 3 líneas cada una): la pantalla Hoy en menos de 100 ms y los
/// reportes anuales en pocos segundos. Para correrlo: `cargo test --release --test reportes --
/// --ignored doscientas_mil`.
#[tokio::test]
#[ignore]
async fn doscientas_mil_ventas() {
    let dir = tempfile::tempdir().unwrap();
    let ruta = dir.path().join("grande.db");
    let escritura = db::abrir(&ruta).await.unwrap();
    generar(&escritura, 200_000).await;
    // Como en la app: los reportes leen por el pool de lectura (con mmap).
    let pool = db::abrir_lectura(&ruta).await.unwrap();

    let inicio = Instant::now();
    let h = reportes::hoy(&pool, &ZonaLocal, Utc::now()).await.unwrap();
    let t_hoy = inicio.elapsed();
    assert!(h.boletas > 0);

    let hoy = ZonaLocal.dia(Utc::now());
    let anio = Rango {
        desde: hoy - chrono::Duration::days(364),
        hasta: hoy,
    };
    // Tope holgado para un PC modesto: en un i5-11300H van de 0,2 a 1,5 s.
    let medir = |nombre: &'static str, t: Instant| {
        let e = t.elapsed();
        println!("{nombre}: {e:?}");
        assert!(e < Duration::from_secs(5), "{nombre} tardó {e:?}");
    };
    let t = Instant::now();
    let v = reportes::ventas(&pool, &ZonaLocal, anio).await.unwrap();
    medir("ventas anual", t);
    let t = Instant::now();
    reportes::productos_vendidos(&pool, &ZonaLocal, anio, None)
        .await
        .unwrap();
    medir("productos anual", t);
    let t = Instant::now();
    reportes::ganancia(&pool, &ZonaLocal, anio).await.unwrap();
    medir("ganancia anual", t);
    let t = Instant::now();
    reportes::fiados(&pool, &ZonaLocal, anio, Utc::now())
        .await
        .unwrap();
    medir("fiados anual", t);
    println!(
        "hoy: {t_hoy:?} ({} boletas hoy, {} en el año)",
        h.boletas, v.boletas
    );
    assert!(v.boletas > 190_000);
    assert!(t_hoy < Duration::from_millis(100), "Hoy tardó {t_hoy:?}");
}

/// Lo mismo en chico, para que corra en la CI: 5.000 ventas.
#[tokio::test]
async fn hoy_con_cinco_mil_ventas_en_el_anio() {
    let dir = tempfile::tempdir().unwrap();
    let ruta = dir.path().join("chica.db");
    let escritura = db::abrir(&ruta).await.unwrap();
    generar(&escritura, 5_000).await;
    let pool = db::abrir_lectura(&ruta).await.unwrap();
    let inicio = Instant::now();
    let h = reportes::hoy(&pool, &ZonaLocal, Utc::now()).await.unwrap();
    let t = inicio.elapsed();
    assert!(h.boletas > 0);
    assert!(t < Duration::from_millis(100), "Hoy tardó {t:?}");
}

/// `n` ventas repartidas en el último año (hasta ahora), con 3 líneas cada una sobre 400
/// productos, por SQL para que sea rápido.
async fn generar(pool: &SqlitePool, n: i64) {
    let _ = DatosProducto::clone;
    let ahora = Utc::now();
    let inicio = ahora - chrono::Duration::days(365);
    let paso_ms = (ahora - inicio).num_milliseconds() / n;
    let mut tx = pool.begin().await.unwrap();
    sqlx::query(
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 400)
         INSERT INTO productos (nombre, precio_venta, costo, stock_mil)
         SELECT 'Producto ' || i, 500 + i * 13, CASE WHEN i % 7 = 0 THEN NULL ELSE 300 + i * 7 END,
                1000000000 FROM n",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    // Un turno por día, como en un negocio real: solo el de hoy queda abierto.
    sqlx::query(
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 366)
         INSERT INTO turnos (id, monto_inicial, estado, cerrado_en, efectivo_esperado,
                             efectivo_contado, diferencia)
         SELECT i, 0, CASE WHEN i = 366 THEN 'abierto' ELSE 'cerrado' END,
                CASE WHEN i = 366 THEN NULL ELSE '2026-01-01T00:00:00.000Z' END,
                0, 0, 0 FROM n",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    let base = Utc.timestamp_millis_opt(inicio.timestamp_millis()).unwrap();
    sqlx::query(
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < ?1)
         INSERT INTO ventas (id, folio, fecha_hora, subtotal, total, medio_pago, estado, turno_id)
         SELECT i, i, strftime('%Y-%m-%dT%H:%M:%fZ', ?2, '+' || (i * ?3 / 1000.0) || ' seconds'),
                0, 0, CASE i % 5 WHEN 0 THEN 'efectivo' WHEN 1 THEN 'debito' WHEN 2 THEN 'credito'
                                 WHEN 3 THEN 'transferencia' ELSE 'efectivo' END,
                CASE WHEN i % 97 = 0 THEN 'anulada' ELSE 'pagada' END,
                1 + i * ?3 / 86400000
         FROM n",
    )
    .bind(n)
    .bind(base.format("%Y-%m-%d %H:%M:%S").to_string())
    .bind(paso_ms)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO venta_items (venta_id, producto_id, nombre, precio_unitario, cantidad_mil,
                                  subtotal, costo_unitario)
         SELECT v.id, p.id, p.nombre, p.precio_venta, 1000 * (1 + v.id % 3),
                p.precio_venta * (1 + v.id % 3), p.costo
         FROM ventas v JOIN (SELECT 0 AS k UNION ALL SELECT 1 UNION ALL SELECT 2) k
         JOIN productos p ON p.id = 1 + (v.id * 7 + k.k * 131) % 400",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE ventas SET subtotal = (SELECT SUM(subtotal) FROM venta_items WHERE venta_id = ventas.id),
                           total = (SELECT SUM(subtotal) FROM venta_items WHERE venta_id = ventas.id)",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    sqlx::query("ANALYZE").execute(pool).await.unwrap();
}
