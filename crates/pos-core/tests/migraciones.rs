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
    assert!(db::version_esquema(&pool).await.unwrap() >= 2);

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
