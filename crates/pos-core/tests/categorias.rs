mod comun;

use pos_core::repositorios::productos::FiltroCategoria;
use pos_core::servicios::{categorias, productos};
use pos_core::{db, Error};

#[tokio::test]
async fn crea_renombra_y_lista_con_productos_activos() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let bebidas = categorias::crear(&pool, "  Bebidas ").await.unwrap();
    let lacteos = categorias::crear(&pool, "Lácteos").await.unwrap();
    assert_eq!((bebidas.nombre.as_str(), bebidas.productos), ("Bebidas", 0));

    let mut d = comun::datos(Some("1"), "Coca-Cola", 1990, 5);
    d.categoria_id = Some(bebidas.id);
    let coca = productos::crear(&pool, &d).await.unwrap();
    assert_eq!(coca.categoria_id, Some(bebidas.id));
    let mut d = comun::datos(Some("2"), "Fanta", 1890, 5);
    d.categoria_id = Some(bebidas.id);
    let fanta = productos::crear(&pool, &d).await.unwrap();
    productos::cambiar_activo(&pool, fanta.id, false)
        .await
        .unwrap();

    let renombrada = categorias::renombrar(&pool, lacteos.id, "Lácteos y huevos")
        .await
        .unwrap();
    assert_eq!(renombrada.nombre, "Lácteos y huevos");

    // Ordenadas por nombre; la cantidad cuenta solo productos activos.
    let lista: Vec<(String, i64)> = categorias::listar(&pool)
        .await
        .unwrap()
        .into_iter()
        .map(|c| (c.nombre, c.productos))
        .collect();
    assert_eq!(
        lista,
        [
            ("Bebidas".to_string(), 1),
            ("Lácteos y huevos".to_string(), 0)
        ]
    );
}

#[tokio::test]
async fn nombre_repetido_sin_importar_mayusculas_es_conflicto() {
    let pool = db::abrir_en_memoria().await.unwrap();
    categorias::crear(&pool, "Bebidas").await.unwrap();
    let otra = categorias::crear(&pool, "Snacks").await.unwrap();

    let repetida = categorias::crear(&pool, "BEBIDAS").await;
    assert!(matches!(repetida, Err(Error::Conflicto(_))), "{repetida:?}");
    let renombrada = categorias::renombrar(&pool, otra.id, "bebidas").await;
    assert!(
        matches!(renombrada, Err(Error::Conflicto(_))),
        "{renombrada:?}"
    );

    let vacia = categorias::crear(&pool, "   ").await;
    assert!(matches!(vacia, Err(Error::Validacion(_))), "{vacia:?}");
    let no_existe = categorias::renombrar(&pool, 999, "X").await;
    assert!(
        matches!(no_existe, Err(Error::NoEncontrado(_))),
        "{no_existe:?}"
    );
}

#[tokio::test]
async fn no_elimina_categorias_con_productos() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let panaderia = categorias::crear(&pool, "Panadería").await.unwrap();
    let vacia = categorias::crear(&pool, "Vacía").await.unwrap();
    let mut d = comun::datos(Some("1"), "Pan", 150, 0);
    d.categoria_id = Some(panaderia.id);
    let pan = productos::crear(&pool, &d).await.unwrap();
    // Un producto desactivado también la ocupa.
    productos::cambiar_activo(&pool, pan.id, false)
        .await
        .unwrap();

    let r = categorias::eliminar(&pool, panaderia.id).await;
    assert!(matches!(r, Err(Error::Conflicto(_))), "{r:?}");

    categorias::eliminar(&pool, vacia.id).await.unwrap();
    let nombres: Vec<String> = categorias::listar(&pool)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.nombre)
        .collect();
    assert_eq!(nombres, ["Panadería"]);
    assert!(matches!(
        categorias::eliminar(&pool, vacia.id).await,
        Err(Error::NoEncontrado(_))
    ));
}

#[tokio::test]
async fn filtra_la_busqueda_por_categoria() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let bebidas = categorias::crear(&pool, "Bebidas").await.unwrap();
    let snacks = categorias::crear(&pool, "Snacks").await.unwrap();
    for (codigo, nombre, categoria) in [
        ("1", "Coca-Cola", Some(bebidas.id)),
        ("2", "Agua mineral", Some(bebidas.id)),
        ("3", "Papas fritas", Some(snacks.id)),
        ("4", "Pilas AA", None),
    ] {
        let mut d = comun::datos(Some(codigo), nombre, 1000, 1);
        d.categoria_id = categoria;
        productos::crear(&pool, &d).await.unwrap();
    }
    let buscar = |texto: &'static str, filtro: FiltroCategoria| {
        let pool = pool.clone();
        async move {
            productos::buscar(&pool, texto, false, filtro, 50, 0)
                .await
                .unwrap()
                .into_iter()
                .map(|p| p.nombre)
                .collect::<Vec<_>>()
        }
    };

    assert_eq!(
        buscar("", FiltroCategoria::Una(bebidas.id)).await,
        ["Agua mineral", "Coca-Cola"]
    );
    assert_eq!(
        buscar("coca", FiltroCategoria::Una(bebidas.id)).await,
        ["Coca-Cola"]
    );
    assert!(buscar("coca", FiltroCategoria::Una(snacks.id))
        .await
        .is_empty());
    assert_eq!(
        buscar("", FiltroCategoria::SinCategoria).await,
        ["Pilas AA"]
    );
    assert_eq!(buscar("", FiltroCategoria::Todas).await.len(), 4);
}

#[tokio::test]
async fn producto_con_categoria_inexistente_se_rechaza() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let mut d = comun::datos(Some("1"), "Pan", 150, 0);
    d.categoria_id = Some(42);
    let r = productos::crear(&pool, &d).await;
    assert!(matches!(r, Err(Error::Validacion(_))), "{r:?}");

    // Editar para quitar la categoría la deja en NULL.
    let c = categorias::crear(&pool, "Panadería").await.unwrap();
    d.categoria_id = Some(c.id);
    let p = productos::crear(&pool, &d).await.unwrap();
    d.categoria_id = None;
    let editado = productos::editar(&pool, p.id, &d).await.unwrap();
    assert_eq!(editado.categoria_id, None);
}
