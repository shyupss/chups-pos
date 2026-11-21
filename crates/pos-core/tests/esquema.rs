use pos_core::{db, servicios};

#[tokio::test]
async fn abre_archivo_migra_y_reporta_estado() {
    let dir = tempfile::tempdir().unwrap();
    let ruta = dir.path().join("prueba.db");

    let pool = db::abrir(&ruta).await.unwrap();
    let estado = servicios::sistema::estado(&pool, "0.1.0", "prueba.db", 12)
        .await
        .unwrap();

    assert_eq!(estado.version_esquema, db::version_app());
    assert_eq!(estado.total_productos, 0);
    assert!(ruta.exists());

    let modo: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(modo, "wal");

    // Abrir de nuevo no vuelve a aplicar migraciones ni falla.
    pool.close().await;
    let pool = db::abrir(&ruta).await.unwrap();
    assert_eq!(db::version_esquema(&pool).await.unwrap(), db::version_app());
}

#[tokio::test]
async fn busqueda_por_nombre_ignora_tildes() {
    let pool = db::abrir_en_memoria().await.unwrap();
    sqlx::query("INSERT INTO productos (codigo_barras, nombre, precio_venta) VALUES (?, ?, ?)")
        .bind("7801610001196")
        .bind("Café en grano 250 g")
        .bind(4990_i64)
        .execute(&pool)
        .await
        .unwrap();

    let encontrados: Vec<String> = sqlx::query_scalar(
        "SELECT p.nombre FROM productos_fts f JOIN productos p ON p.id = f.rowid
         WHERE productos_fts MATCH ?",
    )
    .bind("cafe*")
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(encontrados, vec!["Café en grano 250 g".to_string()]);
}

#[tokio::test]
async fn venta_fiada_exige_cliente() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let resultado = sqlx::query(
        "INSERT INTO ventas (folio, subtotal, total, medio_pago, estado)
         VALUES (1, 1000, 1000, 'fiado', 'fiada')",
    )
    .execute(&pool)
    .await;

    assert!(
        resultado.is_err(),
        "una venta fiada sin cliente debe rechazarse"
    );
}

#[tokio::test]
async fn codigo_de_barras_es_unico() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let insertar =
        "INSERT INTO productos (codigo_barras, nombre, precio_venta) VALUES ('123', 'A', 100)";
    sqlx::query(insertar).execute(&pool).await.unwrap();
    assert!(sqlx::query(insertar).execute(&pool).await.is_err());
}

#[test]
fn la_app_trae_el_esquema_7() {
    assert_eq!(db::version_app(), 7);
}

#[tokio::test]
async fn el_pool_de_lectura_no_escribe() {
    let dir = tempfile::tempdir().unwrap();
    let ruta = dir.path().join("lectura.db");
    let pool = db::abrir(&ruta).await.unwrap();
    sqlx::query("INSERT INTO productos (nombre, precio_venta) VALUES ('Leche', 1000)")
        .execute(&pool)
        .await
        .unwrap();
    let lectura = db::abrir_lectura(&ruta).await.unwrap();
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM productos")
        .fetch_one(&lectura)
        .await
        .unwrap();
    assert_eq!(n, 1);
    assert!(sqlx::query("UPDATE productos SET precio_venta = 1")
        .execute(&lectura)
        .await
        .is_err());
    let mmap: i64 = sqlx::query_scalar("PRAGMA mmap_size")
        .fetch_one(&lectura)
        .await
        .unwrap();
    assert!(mmap > 0);
    // El pool que vende no usa mmap.
    let mmap: i64 = sqlx::query_scalar("PRAGMA mmap_size")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(mmap, 0);
}
