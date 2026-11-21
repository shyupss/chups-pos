mod comun;

use pos_core::repositorios::productos::FiltroCategoria::Todas;
use pos_core::servicios::productos::{self as srv};
use pos_core::{db, Error};

#[tokio::test]
async fn crea_con_stock_inicial_y_movimiento() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let p = comun::crear(&pool, "780123", "Leche entera 1L", 1190, 24).await;

    assert_eq!(p.stock_mil, 24_000);
    assert!(p.activo);
    let (tipo, cantidad): (String, i64) =
        sqlx::query_as("SELECT tipo, cantidad_mil FROM movimientos_stock WHERE producto_id = ?")
            .bind(p.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((tipo.as_str(), cantidad), ("ajuste", 24_000));
}

#[tokio::test]
async fn rechaza_datos_invalidos_y_codigo_repetido() {
    let pool = db::abrir_en_memoria().await.unwrap();
    comun::crear(&pool, "111", "Pan", 100, 0).await;

    let vacio = srv::crear(&pool, &comun::datos(None, "   ", 100, 0)).await;
    assert!(matches!(vacio, Err(Error::Validacion(_))));

    let negativo = srv::crear(&pool, &comun::datos(None, "X", -1, 0)).await;
    assert!(matches!(negativo, Err(Error::Validacion(_))));

    let repetido = srv::crear(&pool, &comun::datos(Some(" 111 "), "Otro", 100, 0)).await;
    assert!(matches!(repetido, Err(Error::Conflicto(_))), "{repetido:?}");
}

#[tokio::test]
async fn editar_precio_deja_historial() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let p = comun::crear(&pool, "222", "Arroz 1kg", 1500, 10).await;

    let mut d = comun::datos(Some("222"), "Arroz grado 1, 1kg", 1690, 999);
    d.stock_minimo_mil = 5000;
    let editado = srv::editar(&pool, p.id, &d).await.unwrap();

    assert_eq!(editado.nombre, "Arroz grado 1, 1kg");
    assert_eq!(editado.precio_venta, 1690);
    assert_eq!(editado.stock_mil, 10_000, "editar no toca el stock");
    let (antes, despues): (i64, i64) = sqlx::query_as(
        "SELECT precio_anterior, precio_nuevo FROM historial_precios WHERE producto_id = ?",
    )
    .bind(p.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((antes, despues), (1500, 1690));
}

#[tokio::test]
async fn busca_por_codigo_nombre_y_respeta_inactivos() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let coca = comun::crear(&pool, "7801610001196", "Coca-Cola 1.5L", 1990, 5).await;
    comun::crear(&pool, "7802820005455", "Té Supremo 20 bolsitas", 1290, 5).await;

    let por_codigo = srv::buscar_por_codigo(&pool, " 7801610001196 ")
        .await
        .unwrap();
    assert_eq!(por_codigo.map(|p| p.id), Some(coca.id));

    let nombres = |v: Vec<pos_core::repositorios::productos::Producto>| {
        v.into_iter().map(|p| p.nombre).collect::<Vec<_>>()
    };
    assert_eq!(
        nombres(
            srv::buscar(&pool, "coca", false, Todas, 50, 0)
                .await
                .unwrap()
        ),
        ["Coca-Cola 1.5L"]
    );
    assert_eq!(
        nombres(
            srv::buscar(&pool, "te sup", false, Todas, 50, 0)
                .await
                .unwrap()
        ),
        ["Té Supremo 20 bolsitas"]
    );
    assert_eq!(
        nombres(
            srv::buscar(&pool, "7802", false, Todas, 50, 0)
                .await
                .unwrap()
        ),
        ["Té Supremo 20 bolsitas"]
    );
    assert_eq!(
        srv::buscar(&pool, "", false, Todas, 50, 0)
            .await
            .unwrap()
            .len(),
        2
    );
    assert!(srv::buscar(&pool, "\"*", false, Todas, 50, 0)
        .await
        .unwrap()
        .is_empty());

    srv::cambiar_activo(&pool, coca.id, false).await.unwrap();
    assert!(srv::buscar_por_codigo(&pool, "7801610001196")
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        srv::buscar(&pool, "coca", false, Todas, 50, 0)
            .await
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        srv::buscar(&pool, "coca", true, Todas, 50, 0)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn pagina_resultados() {
    let pool = db::abrir_en_memoria().await.unwrap();
    for i in 0..25 {
        comun::crear(
            &pool,
            &format!("C{i:03}"),
            &format!("Producto {i:03}"),
            100,
            0,
        )
        .await;
    }
    let primera = srv::buscar(&pool, "", false, Todas, 10, 0).await.unwrap();
    let tercera = srv::buscar(&pool, "", false, Todas, 10, 20).await.unwrap();
    assert_eq!(primera.len(), 10);
    assert_eq!(primera[0].nombre, "Producto 000");
    assert_eq!(tercera.len(), 5);
    assert_eq!(tercera[0].nombre, "Producto 020");
}
