// Una base creada con la fase 1 (solo la migración 0001) debe abrir con la versión nueva
// y conservar sus datos, con las cantidades pasadas a milésimas.

use std::path::Path;

use pos_core::db;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const MIGRACION_0001: &str = "0001_esquema_inicial.sql";

#[tokio::test]
async fn migracion_0002_convierte_datos_de_la_fase_1() {
    let dir = tempfile::tempdir().unwrap();
    let ruta = dir.path().join("fase1.db");

    // Base de la fase 1: solo la migración 0001, copiada tal cual (mismo checksum).
    let solo_0001 = dir.path().join("migraciones");
    std::fs::create_dir(&solo_0001).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations")
            .join(MIGRACION_0001),
        solo_0001.join(MIGRACION_0001),
    )
    .unwrap();
    let opciones = SqliteConnectOptions::new()
        .filename(&ruta)
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opciones)
        .await
        .unwrap();
    Migrator::new(solo_0001.as_path())
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();

    // Un producto con stock 5 y una venta de 2 unidades, como lo dejaba la fase 1.
    sqlx::query(
        "INSERT INTO productos (id, codigo_barras, nombre, precio_venta, stock, stock_minimo)
         VALUES (1, '780', 'Leche', 1190, 5, 2)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ventas (id, folio, subtotal, total, medio_pago, monto_recibido, vuelto, estado)
         VALUES (1, 1, 2380, 2380, 'efectivo', 3000, 620, 'pagada')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO venta_items (venta_id, producto_id, nombre, precio_unitario, cantidad, subtotal)
         VALUES (1, 1, 'Leche', 1190, 2, 2380)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO movimientos_stock (producto_id, tipo, cantidad, stock_resultante, venta_id)
         VALUES (1, 'venta', -2, 3, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE productos SET stock = 3 WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    // La versión nueva abre el mismo archivo y aplica el resto de las migraciones.
    let pool = db::abrir(&ruta).await.unwrap();
    assert_eq!(db::version_esquema(&pool).await.unwrap(), db::version_app());

    let (unidad, stock_mil, minimo_mil): (String, i64, i64) =
        sqlx::query_as("SELECT unidad, stock_mil, stock_minimo_mil FROM productos WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (unidad.as_str(), stock_mil, minimo_mil),
        ("unidad", 3000, 2000)
    );

    let cantidad_mil: i64 = sqlx::query_scalar("SELECT cantidad_mil FROM venta_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cantidad_mil, 2000);

    let movimiento: (i64, i64) =
        sqlx::query_as("SELECT cantidad_mil, stock_resultante_mil FROM movimientos_stock")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(movimiento, (-2000, 3000));

    // El producto sigue funcionando con los servicios nuevos.
    let p = pos_core::servicios::productos::buscar_por_codigo(&pool, "780")
        .await
        .unwrap()
        .unwrap();
    assert_eq!((p.nombre.as_str(), p.stock_mil), ("Leche", 3000));
}

/// Base con las migraciones hasta `ultima` (copiadas tal cual, mismo checksum), sin abrir con
/// la versión actual.
async fn base_hasta(dir: &Path, ultima: &str) -> sqlx::SqlitePool {
    let origen = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let destino = dir.join("migraciones");
    std::fs::create_dir(&destino).unwrap();
    let mut archivos: Vec<_> = std::fs::read_dir(&origen)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.as_str() <= ultima)
        .collect();
    archivos.sort();
    for nombre in &archivos {
        std::fs::copy(origen.join(nombre), destino.join(nombre)).unwrap();
    }
    let opciones = SqliteConnectOptions::new()
        .filename(dir.join("base.db"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opciones)
        .await
        .unwrap();
    Migrator::new(destino.as_path())
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    pool
}

// Una base de la fase 3 (migraciones 0001 a 0004) abre con la fase 4: conserva todo, sus ventas
// quedan sin turno y se puede seguir vendiendo con folios correlativos una vez abierta la caja.
#[tokio::test]
async fn migracion_0005_conserva_la_base_de_la_fase_3() {
    let dir = tempfile::tempdir().unwrap();
    let pool = base_hasta(dir.path(), "0004_boletas.sql").await;
    sqlx::query(
        "UPDATE negocio SET nombre = 'Almacén Don Pepe' WHERE id = 1;
         INSERT INTO config (clave, valor) VALUES ('impresora', 'Archivo (pruebas)');
         INSERT INTO productos (id, codigo_barras, nombre, unidad, precio_venta, stock_mil)
             VALUES (1, 'P', 'Pan', 'kg', 2300, 9468);
         INSERT INTO ventas (id, folio, subtotal, redondeo, total, medio_pago, monto_recibido,
                             vuelto, estado)
             VALUES (1, 41, 1224, -4, 1220, 'efectivo', 2000, 780, 'pagada');
         INSERT INTO venta_items (venta_id, producto_id, nombre, unidad, precio_unitario,
                                  cantidad_mil, subtotal)
             VALUES (1, 1, 'Pan', 'kg', 2300, 532, 1224);
         INSERT INTO movimientos_stock (producto_id, tipo, cantidad_mil, stock_resultante_mil,
                                        venta_id)
             VALUES (1, 'venta', -532, 9468, 1);",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    let pool = db::abrir(&dir.path().join("base.db")).await.unwrap();
    assert_eq!(db::version_esquema(&pool).await.unwrap(), db::version_app());

    let (turno, devuelto, devolucion, anulada): (Option<i64>, i64, Option<String>, Option<String>) =
        sqlx::query_as("SELECT turno_id, monto_devuelto, devolucion, anulada_en FROM ventas")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (turno, devuelto, devolucion, anulada),
        (None, 0, None, None)
    );

    // La boleta antigua se sigue armando igual y la configuración no se perdió.
    let b = pos_core::boleta::armar(&pool, 1).await.unwrap();
    assert_eq!((b.folio, b.total, b.lineas.len()), (41, 1220, 1));
    let cfg = pos_core::servicios::config::obtener(&pool).await.unwrap();
    assert_eq!(cfg.impresora.as_deref(), Some("Archivo (pruebas)"));
    assert!(
        !pos_core::servicios::seguridad::estado(&pool)
            .await
            .unwrap()
            .tiene_pin
    );

    // Sin caja abierta no se vende; al abrirla, el folio sigue en 42.
    use pos_core::servicios::ventas::{self, ItemPedido, MedioPago, NuevaVenta};
    let venta = NuevaVenta {
        items: vec![ItemPedido {
            producto_id: 1,
            cantidad_mil: 500,
        }],
        medio_pago: MedioPago::Debito,
        monto_recibido: None,
        descuento: 0,
        cliente_id: None,
    };
    assert!(ventas::confirmar(&pool, &venta).await.is_err());
    pos_core::servicios::turnos::abrir(&pool, 10_000)
        .await
        .unwrap();
    assert_eq!(ventas::confirmar(&pool, &venta).await.unwrap().folio, 42);

    // Las ventas y los movimientos ya no se pueden borrar.
    assert!(sqlx::query("DELETE FROM ventas WHERE id = 1")
        .execute(&pool)
        .await
        .is_err());
    assert!(sqlx::query("DELETE FROM movimientos_stock")
        .execute(&pool)
        .await
        .is_err());
}

// Una base de la fase 4 (0001 a 0005) con turnos, una fiada con devolución y una anulación en
// efectivo abre con la fase 5 sin perder nada, y los datos nuevos quedan coherentes.
#[tokio::test]
async fn migracion_0006_conserva_la_base_de_la_fase_4() {
    let dir = tempfile::tempdir().unwrap();
    let pool = base_hasta(dir.path(), "0005_turnos_y_devoluciones.sql").await;
    sqlx::query(
        "INSERT INTO productos (id, codigo_barras, nombre, precio_venta, stock_mil, stock_minimo_mil)
             VALUES (1, 'L', 'Leche', 1000, 8000, 2000);
         INSERT INTO clientes (id, nombre, telefono) VALUES (1, 'Rosa', '+56 9 1111 2222');
         INSERT INTO turnos (id, monto_inicial, estado, cerrado_en, efectivo_esperado,
                             efectivo_contado, diferencia)
             VALUES (1, 10000, 'cerrado', '2026-11-20T23:00:00.000Z', 8780, 8780, 0);
         INSERT INTO turnos (id, monto_inicial) VALUES (2, 5000);
         INSERT INTO ventas (id, folio, subtotal, total, medio_pago, estado, cliente_id, turno_id,
                             devolucion, monto_devuelto)
             VALUES (1, 1, 3000, 3000, 'fiado', 'fiada', 1, 1, 'parcial', 1000);
         INSERT INTO venta_items (id, venta_id, producto_id, nombre, precio_unitario, cantidad_mil,
                                  subtotal)
             VALUES (1, 1, 1, 'Leche', 1000, 3000, 3000);
         INSERT INTO devoluciones (id, venta_id, turno_id, monto, motivo)
             VALUES (1, 1, 2, 1000, 'Venía mala');
         INSERT INTO devolucion_items (devolucion_id, venta_item_id, cantidad_mil, monto)
             VALUES (1, 1, 1000, 1000);
         INSERT INTO ventas (id, folio, subtotal, total, medio_pago, estado, turno_id, anulada_en,
                             motivo_anulacion, turno_anulacion_id, medio_reembolso_anulacion,
                             reembolso_anulacion)
             VALUES (2, 2, 1224, 1224, 'debito', 'anulada', 1, '2026-11-20T22:00:00.000Z',
                     'Error', 1, 'efectivo', 1220);
         INSERT INTO movimientos_caja (turno_id, tipo, monto, motivo)
             VALUES (2, 'retiro', 500, 'Bolsas');",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    let pool = db::abrir(&dir.path().join("base.db")).await.unwrap();
    assert_eq!(db::version_esquema(&pool).await.unwrap(), db::version_app());

    // La devolución de la fiada bajó la deuda completa; el reembolso de la anulación quedó a la decena.
    let rebaja: i64 = sqlx::query_scalar("SELECT rebaja_deuda FROM devoluciones WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rebaja, 1000);
    let redondeo: Option<i64> =
        sqlx::query_scalar("SELECT redondeo_anulacion FROM ventas WHERE id = 2")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(redondeo, Some(-4));
    let (creado, limite): (Option<String>, Option<i64>) =
        sqlx::query_as("SELECT creado_en, limite_credito FROM clientes WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(creado.is_some());
    assert_eq!(limite, None);

    // El turno cerrado no cambió y el abierto sigue abierto con su retiro.
    use pos_core::servicios::turnos;
    let cerrado = turnos::resumen(&pool, 1).await.unwrap();
    assert_eq!(cerrado.turno.efectivo_esperado, Some(8780));
    assert_eq!(cerrado.redondeo_reembolsos, -4);
    let abierto = turnos::estado_caja(&pool).await.unwrap().turno.unwrap();
    assert_eq!(abierto.id, 2);
    assert_eq!(
        turnos::resumen(&pool, 2).await.unwrap().efectivo.retiros,
        500
    );

    // Historial y detalle de la venta fiada se siguen leyendo.
    let d = pos_core::servicios::historial::detalle(&pool, 1)
        .await
        .unwrap();
    assert_eq!(
        (d.folio, d.monto_devuelto, d.devoluciones.len()),
        (1, 1000, 1)
    );

    // Los abonos y los clientes con ventas no se pueden borrar.
    sqlx::query("INSERT INTO abonos (venta_id, monto, medio_pago) VALUES (1, 100, 'efectivo')")
        .execute(&pool)
        .await
        .unwrap();
    assert!(sqlx::query("DELETE FROM abonos")
        .execute(&pool)
        .await
        .is_err());
    assert!(sqlx::query("DELETE FROM clientes WHERE id = 1")
        .execute(&pool)
        .await
        .is_err());
}

// Una base de la fase 5 abre con la fase 6: cada línea vendida toma el costo actual de su producto
// marcado como estimado (las de productos sin costo quedan en NULL) y no se pierde nada.
#[tokio::test]
async fn migracion_0007_rellena_costos_estimados_desde_la_fase_5() {
    let dir = tempfile::tempdir().unwrap();
    let pool = base_hasta(dir.path(), "0006_inventario_y_fiados.sql").await;
    sqlx::query(
        "INSERT INTO productos (id, codigo_barras, nombre, precio_venta, costo, stock_mil)
             VALUES (1, 'L', 'Leche', 1190, 850, 8000), (2, 'P', 'Pan', 2300, NULL, 5000);
         UPDATE productos SET unidad = 'kg' WHERE id = 2;
         INSERT INTO clientes (id, nombre) VALUES (1, 'Rosa');
         INSERT INTO turnos (id, monto_inicial) VALUES (1, 0);
         INSERT INTO ventas (id, folio, subtotal, total, medio_pago, estado, turno_id)
             VALUES (1, 1, 3530, 3530, 'debito', 'pagada', 1);
         INSERT INTO venta_items (venta_id, producto_id, nombre, unidad, precio_unitario,
                                  cantidad_mil, subtotal)
             VALUES (1, 1, 'Leche', 'unidad', 1190, 2000, 2380),
                    (1, 2, 'Pan', 'kg', 2300, 500, 1150);
         INSERT INTO ventas (id, folio, subtotal, total, medio_pago, estado, cliente_id, turno_id)
             VALUES (2, 2, 1190, 1190, 'fiado', 'fiada', 1, 1);
         INSERT INTO venta_items (venta_id, producto_id, nombre, precio_unitario, cantidad_mil,
                                  subtotal)
             VALUES (2, 1, 'Leche', 1190, 1000, 1190);
         INSERT INTO pagos_cliente (id, cliente_id, monto, medio_pago, turno_id)
             VALUES (1, 1, 500, 'efectivo', 1);
         INSERT INTO abonos (venta_id, monto, medio_pago, pago_id, turno_id)
             VALUES (2, 500, 'efectivo', 1, 1);",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    let pool = db::abrir(&dir.path().join("base.db")).await.unwrap();
    assert_eq!(db::version_esquema(&pool).await.unwrap(), db::version_app());
    let costos: Vec<(i64, Option<i64>, bool)> = sqlx::query_as(
        "SELECT producto_id, costo_unitario, costo_estimado FROM venta_items ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        costos,
        [(1, Some(850), true), (2, None, false), (1, Some(850), true)]
    );

    // Nada se perdió: ventas, líneas, deuda y pagos siguen ahí.
    let cuentas: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM ventas), (SELECT COUNT(*) FROM venta_items),
                (SELECT COUNT(*) FROM pagos_cliente), (SELECT COUNT(*) FROM abonos)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(cuentas, (2, 3, 1, 1));
    assert_eq!(
        pos_core::servicios::clientes::obtener(&pool, 1)
            .await
            .unwrap()
            .deuda,
        690
    );

    // Una venta nueva guarda el costo del momento, sin marcarlo como estimado.
    use pos_core::servicios::ventas::{self, ItemPedido, MedioPago, NuevaVenta};
    sqlx::query("UPDATE productos SET costo = 900 WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    let v = ventas::confirmar(
        &pool,
        &NuevaVenta {
            items: vec![
                ItemPedido {
                    producto_id: 1,
                    cantidad_mil: 1000,
                },
                ItemPedido {
                    producto_id: 2,
                    cantidad_mil: 250,
                },
            ],
            medio_pago: MedioPago::Debito,
            monto_recibido: None,
            descuento: 0,
            cliente_id: None,
        },
    )
    .await
    .unwrap();
    let nuevas: Vec<(Option<i64>, bool)> = sqlx::query_as(
        "SELECT costo_unitario, costo_estimado FROM venta_items WHERE venta_id = ? ORDER BY id",
    )
    .bind(v.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(nuevas, [(Some(900), false), (None, false)]);
}
