// Importar y exportar Excel: ida y vuelta de productos, detección de columnas, filas con error,
// actualización por código de barras, stock como movimientos 'ajuste', PIN, archivo cambiado,
// clientes, 5.000 productos y la exportación de ventas leída de vuelta.

mod comun;

use std::path::Path;
use std::time::Instant;

use calamine::{open_workbook_auto, Data, Reader};
use chrono::Utc;
use pos_core::repositorios::productos::Unidad;
use pos_core::servicios::clientes::{self, DatosCliente};
use pos_core::servicios::exportaciones::{self, Exportacion};
use pos_core::servicios::importacion::{self, CampoProducto, Tipo};
use pos_core::servicios::productos::{self, DatosProducto};
use pos_core::servicios::reportes::Rango;
use pos_core::servicios::seguridad::{self, ControlPin};
use pos_core::servicios::ventas::{self, ItemPedido, MedioPago, NuevaVenta};
use pos_core::zona::{Zona, ZonaLocal};
use pos_core::{db, Error, FalloPin};
use sqlx::SqlitePool;

async fn categoria(pool: &SqlitePool, nombre: &str) -> i64 {
    pos_core::servicios::categorias::crear(pool, nombre)
        .await
        .unwrap()
        .id
}

#[derive(Debug, PartialEq, Eq, sqlx::FromRow)]
struct Fila {
    codigo_barras: Option<String>,
    nombre: String,
    categoria: Option<String>,
    unidad: String,
    precio_venta: i64,
    costo: Option<i64>,
    stock_mil: i64,
    stock_minimo_mil: i64,
}

async fn catalogo(pool: &SqlitePool) -> Vec<Fila> {
    sqlx::query_as(
        "SELECT p.codigo_barras, p.nombre, c.nombre AS categoria, p.unidad, p.precio_venta,
                p.costo, p.stock_mil, p.stock_minimo_mil
         FROM productos p LEFT JOIN categorias c ON c.id = p.categoria_id ORDER BY p.nombre",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

fn escribir(dir: &Path, nombre: &str, texto: &str) -> String {
    let ruta = dir.join(nombre);
    std::fs::write(&ruta, texto).unwrap();
    ruta.to_string_lossy().into_owned()
}

#[tokio::test]
async fn exportar_e_importar_productos_da_lo_mismo() {
    let origen = db::abrir_en_memoria().await.unwrap();
    let lacteos = categoria(&origen, "Lácteos").await;
    let mut leche = comun::datos(Some("7801610001196"), "Leche entera 1 L", 1190, 24);
    leche.categoria_id = Some(lacteos);
    leche.costo = Some(850);
    leche.stock_minimo_mil = 6000;
    productos::crear(&origen, &leche).await.unwrap();
    let mut queso = comun::datos(Some("2002"), "Queso mantecoso", 10_490, 0);
    queso.unidad = Unidad::Kg;
    queso.stock_inicial_mil = 2_532;
    queso.categoria_id = Some(lacteos);
    productos::crear(&origen, &queso).await.unwrap();
    productos::crear(
        &origen,
        &comun::datos(None, "Bolsa de pan (sin código)", 50, 0),
    )
    .await
    .unwrap();

    let dir = tempfile::tempdir().unwrap();
    let xlsx = dir.path().join("productos.xlsx");
    exportaciones::exportar(
        &origen,
        &ZonaLocal,
        Exportacion::Productos,
        None,
        xlsx.clone(),
        Utc::now(),
    )
    .await
    .unwrap();

    // En una base vacía: se detectan las columnas y todo es nuevo.
    let destino = db::abrir_en_memoria().await.unwrap();
    let a = importacion::abrir(&xlsx).unwrap();
    assert_eq!(a.filas, 3);
    assert_eq!(
        &a.productos[..8],
        [
            Some(CampoProducto::Codigo),
            Some(CampoProducto::Nombre),
            Some(CampoProducto::Categoria),
            Some(CampoProducto::Unidad),
            Some(CampoProducto::Precio),
            Some(CampoProducto::Costo),
            Some(CampoProducto::Stock),
            Some(CampoProducto::StockMinimo),
        ]
    );
    let v = importacion::previsualizar_productos(&destino, &a.ruta, &a.productos)
        .await
        .unwrap();
    assert_eq!(
        (v.nuevas.len(), v.actualizan.len(), v.errores.len()),
        (3, 0, 0)
    );
    assert_eq!(v.categorias_nuevas, ["Lácteos"]);
    let r = importacion::importar_productos(
        &destino,
        &ControlPin::new(),
        &a.ruta,
        &a.productos,
        &v.huella,
        None,
    )
    .await
    .unwrap();
    assert_eq!((r.creados, r.categorias_creadas), (3, 1));
    assert_eq!(catalogo(&destino).await, catalogo(&origen).await);

    // El stock entró como 'ajuste' con motivo "Importación", nunca escrito directo.
    let movs: Vec<(String, i64, Option<String>)> =
        sqlx::query_as("SELECT tipo, cantidad_mil, motivo FROM movimientos_stock ORDER BY id")
            .fetch_all(&destino)
            .await
            .unwrap();
    assert_eq!(
        movs,
        [
            ("ajuste".into(), 24_000, Some("Importación".into())),
            ("ajuste".into(), 2_532, Some("Importación".into())),
        ]
    );

    // Importar lo mismo otra vez no cambia nada (salvo el sin código, que es nuevo otra vez).
    let v = importacion::previsualizar_productos(&destino, &a.ruta, &a.productos)
        .await
        .unwrap();
    assert_eq!(
        (v.nuevas.len(), v.actualizan.len(), v.sin_cambios),
        (1, 0, 2)
    );
}

#[tokio::test]
async fn filas_con_error_y_actualizacion_por_codigo() {
    let pool = comun::con_caja().await;
    let mut d = comun::datos(Some("780"), "Leche", 1190, 10);
    d.costo = Some(800);
    let leche = productos::crear(&pool, &d).await.unwrap();
    comun::crear(&pool, "790", "Huevos", 300, 30).await;
    let dir = tempfile::tempdir().unwrap();
    let ruta = escribir(
        dir.path(),
        "productos.csv",
        "Código;Nombre;Categoría;Unidad;Precio;Costo;Stock;Stock mínimo\n\
         780;;;;1.290;;14;\n\
         781;Pan amasado;Panadería;kg;2300;;1,5;\n\
         782;;;;990;;;\n\
         783;Arroz;;;abc;;;\n\
         781;Pan otra vez;;;100;;;\n\
         784;Bebida;;litro;1500;;;\n\
         785;Galletas;;unidad;890;;2,5;\n\
         790;;;kg;;;;\n\
         ;Sin código;;;500;;3;\n",
    );
    let a = importacion::abrir(Path::new(&ruta)).unwrap();
    let v = importacion::previsualizar_productos(&pool, &ruta, &a.productos)
        .await
        .unwrap();
    let errores: Vec<(usize, &str)> = v
        .errores
        .iter()
        .map(|e| (e.fila, e.motivo.as_str()))
        .collect();
    assert_eq!(errores.len(), 6, "{errores:?}");
    assert_eq!(errores[0].0, 4);
    assert!(errores[0].1.contains("Falta el nombre"));
    assert!(errores[1].1.contains("«abc» no es un monto"));
    assert!(errores[2].1.contains("ya está en la fila 3"));
    assert!(errores[3].1.contains("unidad «litro»"));
    assert!(errores[4].1.contains("entero de unidades"));
    assert!(errores[5].1.contains("se vende por unidad"));
    assert_eq!(v.nuevas.len(), 2);
    assert_eq!(v.actualizan.len(), 1);
    assert_eq!(
        v.actualizan[0].cambios,
        ["Precio $1.190 → $1.290", "Stock 10 → 14"]
    );
    assert_eq!(v.categorias_nuevas, ["Panadería"]);

    // Con PIN: sin él no se importa.
    let control = ControlPin::new();
    seguridad::crear_pin(&pool, "4321").await.unwrap();
    assert!(matches!(
        importacion::importar_productos(&pool, &control, &ruta, &a.productos, &v.huella, None)
            .await,
        Err(Error::Pin(FalloPin::Requerido))
    ));
    let r = importacion::importar_productos(
        &pool,
        &control,
        &ruta,
        &a.productos,
        &v.huella,
        Some("4321"),
    )
    .await
    .unwrap();
    assert_eq!((r.creados, r.actualizados, r.omitidos), (2, 1, 6));
    let l = productos::buscar_por_codigo(&pool, "780")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (l.precio_venta, l.costo, l.stock_mil),
        (1290, Some(800), 14_000)
    );
    let (delta, motivo): (i64, Option<String>) = sqlx::query_as(
        "SELECT cantidad_mil, motivo FROM movimientos_stock WHERE producto_id = ? ORDER BY id DESC",
    )
    .bind(leche.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((delta, motivo.as_deref()), (4000, Some("Importación")));
    let precios: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM historial_precios WHERE producto_id = ?")
            .bind(leche.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(precios, 1);

    // Si el archivo cambia después de la vista previa, no se importa.
    let v = importacion::previsualizar_productos(&pool, &ruta, &a.productos)
        .await
        .unwrap();
    escribir(
        dir.path(),
        "productos.csv",
        "Código;Nombre;Precio\n999;Otro;100\n",
    );
    assert!(matches!(
        importacion::importar_productos(&pool, &control, &ruta, &a.productos, &v.huella, Some("4321")).await,
        Err(Error::Validacion(m)) if m.contains("cambió")
    ));
}

#[tokio::test]
async fn importar_clientes_por_rut_o_nombre_sin_deudas() {
    let pool = db::abrir_en_memoria().await.unwrap();
    clientes::crear(
        &pool,
        &DatosCliente {
            nombre: "María González".into(),
            telefono: None,
            rut: Some("11111111-1".into()),
            direccion: None,
            nota: None,
            limite_credito: Some(20_000),
        },
    )
    .await
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let ruta = escribir(
        dir.path(),
        "clientes.csv",
        "Nombre,RUT,Celular,Cupo,Deuda\n\
         Maria G.,11.111.111-1,+56 9 1111 1111,30000,5000\n\
         Juan Pérez,,+56 9 5555 1234,,\n\
         Rosa,12.345.678-0,,,\n",
    );
    let a = importacion::abrir(Path::new(&ruta)).unwrap();
    // "Deuda" no se asigna a nada: las deudas solo nacen de ventas fiadas.
    assert_eq!(a.clientes[4], None);
    let v = importacion::previsualizar_clientes(&pool, &ruta, &a.clientes)
        .await
        .unwrap();
    assert_eq!(
        (v.nuevas.len(), v.actualizan.len(), v.errores.len()),
        (1, 1, 1)
    );
    assert!(
        v.errores[0].motivo.contains("RUT"),
        "{}",
        v.errores[0].motivo
    );
    assert_eq!(
        v.actualizan[0].cambios.len(),
        3,
        "{:?}",
        v.actualizan[0].cambios
    );
    let r = importacion::importar_clientes(
        &pool,
        &ControlPin::new(),
        &ruta,
        &a.clientes,
        &v.huella,
        None,
    )
    .await
    .unwrap();
    assert_eq!((r.creados, r.actualizados, r.omitidos), (1, 1, 1));
    let todos = clientes::buscar(&pool, "", 10).await.unwrap();
    assert_eq!(todos.len(), 2);
    assert!(todos.iter().all(|c| c.deuda == 0));
    let maria = todos
        .iter()
        .find(|c| c.rut.as_deref() == Some("11.111.111-1"))
        .unwrap();
    assert_eq!(
        (maria.nombre.as_str(), maria.limite_credito),
        ("Maria G.", Some(30_000))
    );
}

#[tokio::test]
async fn cinco_mil_productos_en_pocos_segundos() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut csv = String::from("Código;Nombre;Categoría;Unidad;Precio;Costo;Stock;Stock mínimo\n");
    for i in 0..5000 {
        csv.push_str(&format!(
            "78{i:011};Producto {i};Categoría {};unidad;{};{};{};5\n",
            i % 20,
            500 + i,
            300 + i,
            i % 50
        ));
    }
    let ruta = escribir(dir.path(), "cinco-mil.csv", &csv);
    let inicio = Instant::now();
    let a = importacion::abrir(Path::new(&ruta)).unwrap();
    let v = importacion::previsualizar_productos(&pool, &ruta, &a.productos)
        .await
        .unwrap();
    let r = importacion::importar_productos(
        &pool,
        &ControlPin::new(),
        &ruta,
        &a.productos,
        &v.huella,
        None,
    )
    .await
    .unwrap();
    let t = inicio.elapsed();
    println!("5.000 productos: {t:?}");
    assert_eq!((r.creados, r.categorias_creadas, r.omitidos), (5000, 20, 0));
    // El límite pedido es para un PC modesto con un build de entrega; en debug se da más margen.
    let limite = if cfg!(debug_assertions) { 30 } else { 5 };
    assert!(t.as_secs() < limite, "tardó {t:?}");
}

#[tokio::test]
async fn la_exportacion_de_ventas_trae_ventas_y_detalle_con_numeros() {
    let pool = comun::con_caja().await;
    let p = comun::crear(&pool, "A", "Leche", 1190, 100).await;
    for _ in 0..3 {
        ventas::confirmar(
            &pool,
            &NuevaVenta {
                items: vec![ItemPedido {
                    producto_id: p.id,
                    cantidad_mil: 2000,
                }],
                medio_pago: MedioPago::Debito,
                monto_recibido: None,
                descuento: 0,
                cliente_id: None,
            },
        )
        .await
        .unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let ruta = dir.path().join("ventas.xlsx");
    let hoy = ZonaLocal.dia(Utc::now());
    exportaciones::exportar(
        &pool,
        &ZonaLocal,
        Exportacion::Ventas,
        Some(Rango {
            desde: hoy,
            hasta: hoy,
        }),
        ruta.clone(),
        Utc::now(),
    )
    .await
    .unwrap();
    let mut libro = open_workbook_auto(&ruta).unwrap();
    assert_eq!(libro.sheet_names(), ["Ventas", "Detalle"]);
    let ventas = libro.worksheet_range("Ventas").unwrap();
    // Título, detalle, una fila en blanco, encabezados y 3 ventas.
    assert_eq!(ventas.height(), 7);
    assert_eq!(ventas.get((3, 0)), Some(&Data::String("Folio".into())));
    // El total es un número, no texto.
    assert_eq!(ventas.get((4, 8)), Some(&Data::Float(2380.0)));
    let detalle = libro.worksheet_range("Detalle").unwrap();
    assert_eq!(detalle.height(), 7);

    // La planilla de ejemplo se puede leer y detecta todas sus columnas.
    let ejemplo = dir.path().join("ejemplo.xlsx");
    exportaciones::planilla_ejemplo(Tipo::Productos, &ejemplo).unwrap();
    let a = importacion::abrir(&ejemplo).unwrap();
    assert_eq!(a.filas, 2);
    assert!(a.productos.iter().all(Option::is_some));
    let v = importacion::previsualizar_productos(&pool, &a.ruta, &a.productos)
        .await
        .unwrap();
    assert_eq!((v.nuevas.len(), v.errores.len()), (2, 0));
    let _ = DatosProducto::clone;
}

#[tokio::test]
async fn exportar_todo_trae_una_hoja_por_cada_tabla_importante() {
    let pool = comun::con_caja().await;
    let p = comun::crear(&pool, "A", "Leche", 1190, 100).await;
    ventas::confirmar(
        &pool,
        &NuevaVenta {
            items: vec![ItemPedido {
                producto_id: p.id,
                cantidad_mil: 2000,
            }],
            medio_pago: MedioPago::Debito,
            monto_recibido: None,
            descuento: 0,
            cliente_id: None,
        },
    )
    .await
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let ruta = dir.path().join("todo.xlsx");
    // Sin rango: es todo el historial, no un período.
    exportaciones::exportar(
        &pool,
        &ZonaLocal,
        Exportacion::Todo,
        None,
        ruta.clone(),
        Utc::now(),
    )
    .await
    .unwrap();
    let mut libro = open_workbook_auto(&ruta).unwrap();
    let hojas = libro.sheet_names();
    for esperada in [
        "Productos",
        "Stock",
        "Clientes",
        "Ventas",
        "Detalle",
        "Movimientos",
        "Compras",
    ] {
        assert!(
            hojas.iter().any(|h| h.starts_with(esperada)),
            "falta una hoja de {esperada}: {hojas:?}"
        );
    }
    let ventas = libro.worksheet_range("Ventas").unwrap();
    assert_eq!(
        ventas.height(),
        5,
        "título, detalle, línea en blanco, encabezados y 1 venta"
    );
    assert_eq!(Exportacion::Todo.nombre_base(), "todos-los-datos");
    assert!(!Exportacion::Todo.usa_rango());
}
