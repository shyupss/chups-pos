// Compras y proveedores: stock, costo, pago con la caja y anulación que compensa.

mod comun;

use pos_core::servicios::compras::{self, FiltrosCompras, LineaCompra, NuevaCompra};
use pos_core::servicios::productos;
use pos_core::servicios::proveedores::{self, DatosProveedor};
use pos_core::servicios::seguridad::{self, ControlPin};
use pos_core::servicios::turnos;
use pos_core::{db, Error, FalloPin};
use sqlx::SqlitePool;

fn datos(nombre: &str, rut: Option<&str>) -> DatosProveedor {
    DatosProveedor {
        nombre: nombre.into(),
        rut: rut.map(str::to_owned),
        telefono: Some("+56 2 2345 6789".into()),
        contacto: Some("Juan".into()),
        nota: None,
    }
}

fn compra(proveedor_id: Option<i64>, lineas: &[(i64, i64, i64)], caja: bool) -> NuevaCompra {
    NuevaCompra {
        proveedor_id,
        documento: Some(" F-1234 ".into()),
        nota: None,
        lineas: lineas
            .iter()
            .map(|&(producto_id, cantidad_mil, costo_unitario)| LineaCompra {
                producto_id,
                cantidad_mil,
                costo_unitario,
            })
            .collect(),
        pagada_con_caja: caja,
    }
}

async fn producto(pool: &SqlitePool, codigo: &str) -> pos_core::repositorios::productos::Producto {
    productos::buscar_por_codigo(pool, codigo)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn proveedores_con_rut_validado_y_desactivar() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let p = proveedores::crear(&pool, &datos("  Distribuidora Sur ", Some("76.123.456-0")))
        .await
        .unwrap();
    assert_eq!(p.nombre, "Distribuidora Sur");
    assert_eq!(p.rut.as_deref(), Some("76.123.456-0"));
    assert!(
        proveedores::crear(&pool, &datos("Otra", Some("76.123.456-1")))
            .await
            .is_err()
    );
    assert!(proveedores::crear(&pool, &datos("   ", None))
        .await
        .is_err());

    let editado = proveedores::editar(&pool, p.id, &datos("Distribuidora Sur Ltda.", None))
        .await
        .unwrap();
    assert_eq!(
        (editado.nombre.as_str(), editado.rut),
        ("Distribuidora Sur Ltda.", None)
    );
    let lista = proveedores::listar(&pool, "sur", false, 50, 0)
        .await
        .unwrap();
    assert_eq!(lista.len(), 1);

    proveedores::cambiar_activo(&pool, p.id, false)
        .await
        .unwrap();
    assert!(proveedores::listar(&pool, "", false, 50, 0)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        proveedores::listar(&pool, "", true, 50, 0)
            .await
            .unwrap()
            .len(),
        1
    );
    // Un proveedor desactivado no recibe compras nuevas.
    let leche = comun::crear(&pool, "L", "Leche", 1190, 0).await;
    assert!(compras::registrar(
        &pool,
        &ControlPin::new(),
        &compra(Some(p.id), &[(leche.id, 1000, 800)], false),
        None
    )
    .await
    .is_err());
}

#[tokio::test]
async fn la_compra_suma_stock_y_actualiza_el_costo() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let prov = proveedores::crear(&pool, &datos("Lácteos del Sur", None))
        .await
        .unwrap();
    let leche = comun::crear(&pool, "L", "Leche", 1190, 4).await;
    let queso = comun::crear_kg(&pool, "Q", "Queso", 10_000, 500).await;

    // 12 leches a $780 y 2,5 kg de queso a $6.990/kg = 9.360 + 17.475.
    let d = compras::registrar(
        &pool,
        &ControlPin::new(),
        &compra(
            Some(prov.id),
            &[(leche.id, 12_000, 780), (queso.id, 2_500, 6_990)],
            false,
        ),
        None,
    )
    .await
    .unwrap();
    assert_eq!(d.compra.total, 9_360 + 17_475);
    assert_eq!(d.compra.documento.as_deref(), Some("F-1234"));
    assert_eq!(d.compra.proveedor.as_deref(), Some("Lácteos del Sur"));
    assert_eq!(d.items.len(), 2);

    let l = producto(&pool, "L").await;
    let q = producto(&pool, "Q").await;
    assert_eq!((l.stock_mil, l.costo), (16_000, Some(780)));
    assert_eq!((q.stock_mil, q.costo), (3_000, Some(6_990)));

    // Movimientos 'compra' con la compra de origen.
    let movs: Vec<(String, i64, Option<i64>)> = sqlx::query_as(
        "SELECT tipo, cantidad_mil, compra_id FROM movimientos_stock WHERE tipo = 'compra' ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        movs,
        [
            ("compra".to_owned(), 12_000, Some(d.compra.id)),
            ("compra".to_owned(), 2_500, Some(d.compra.id)),
        ]
    );

    // Validaciones: sin líneas, duplicadas, media unidad, costo negativo.
    let c = ControlPin::new();
    assert!(
        compras::registrar(&pool, &c, &compra(None, &[], false), None)
            .await
            .is_err()
    );
    assert!(compras::registrar(
        &pool,
        &c,
        &compra(None, &[(leche.id, 1000, 1), (leche.id, 1000, 1)], false),
        None
    )
    .await
    .is_err());
    assert!(compras::registrar(
        &pool,
        &c,
        &compra(None, &[(leche.id, 1500, 780)], false),
        None
    )
    .await
    .is_err());
    assert!(compras::registrar(
        &pool,
        &c,
        &compra(None, &[(leche.id, 1000, -1)], false),
        None
    )
    .await
    .is_err());

    let lista = compras::listar(
        &pool,
        &FiltrosCompras {
            proveedor_id: Some(prov.id),
            ..Default::default()
        },
        50,
        0,
    )
    .await
    .unwrap();
    assert_eq!(lista.len(), 1);
}

#[tokio::test]
async fn pagada_con_la_caja_hace_un_retiro_del_turno() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let prov = proveedores::crear(&pool, &datos("Panadería", None))
        .await
        .unwrap();
    let pan = comun::crear(&pool, "P", "Pan", 250, 0).await;
    let c = ControlPin::new();
    let pedido = compra(Some(prov.id), &[(pan.id, 40_000, 120)], true);

    // Sin caja abierta, no.
    assert!(matches!(
        compras::registrar(&pool, &c, &pedido, None).await,
        Err(Error::Validacion(m)) if m.starts_with("Abre la caja")
    ));
    let t = turnos::abrir(&pool, 3000).await.unwrap();
    // No hay tanto efectivo: nada se guarda.
    assert!(matches!(
        compras::registrar(&pool, &c, &pedido, None).await,
        Err(Error::Validacion(m)) if m.contains("debería haber")
    ));
    assert_eq!(producto(&pool, "P").await.stock_mil, 0);

    turnos::registrar_movimiento(
        &pool,
        &c,
        turnos::TipoMovimiento::Ingreso,
        5000,
        "Sencillo",
        None,
    )
    .await
    .unwrap();
    let d = compras::registrar(&pool, &c, &pedido, None).await.unwrap();
    assert!(d.compra.pagada_con_caja);
    let r = turnos::resumen(&pool, t.id).await.unwrap();
    assert_eq!(r.efectivo.retiros, 4800);
    assert_eq!(r.efectivo.esperado, 3000 + 5000 - 4800);
    assert_eq!(r.movimientos.last().unwrap().motivo, "Compra a Panadería");

    // Con PIN, el pago con la caja respeta el PIN de los retiros.
    seguridad::crear_pin(&pool, "4821").await.unwrap();
    let otra = compra(None, &[(pan.id, 1000, 100)], true);
    assert!(matches!(
        compras::registrar(&pool, &c, &otra, None).await,
        Err(Error::Pin(FalloPin::Requerido))
    ));
    let d = compras::registrar(&pool, &c, &otra, Some("4821"))
        .await
        .unwrap();
    assert_eq!(
        turnos::resumen(&pool, t.id)
            .await
            .unwrap()
            .movimientos
            .last()
            .unwrap()
            .motivo,
        "Compra sin proveedor"
    );
    // Sin pagar con la caja, no pide nada.
    compras::registrar(
        &pool,
        &c,
        &compra(None, &[(pan.id, 1000, 100)], false),
        None,
    )
    .await
    .unwrap();
    let _ = d;
}

#[tokio::test]
async fn anular_la_compra_compensa_stock_y_caja() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let c = ControlPin::new();
    let leche = comun::crear(&pool, "L", "Leche", 1190, 0).await;
    let t = turnos::abrir(&pool, 20_000).await.unwrap();
    let d = compras::registrar(
        &pool,
        &c,
        &compra(None, &[(leche.id, 10_000, 800)], true),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        turnos::resumen(&pool, t.id)
            .await
            .unwrap()
            .efectivo
            .esperado,
        12_000
    );

    // Se vendieron 3: anular dejaría el stock en −3, y el detalle lo advierte.
    sqlx::query("UPDATE productos SET stock_mil = 7000 WHERE id = ?")
        .bind(leche.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        compras::detalle(&pool, d.compra.id)
            .await
            .unwrap()
            .quedarian_negativos,
        ["Leche"]
    );
    // Con PIN protegido, lo pide.
    seguridad::crear_pin(&pool, "4821").await.unwrap();
    assert!(matches!(
        compras::anular(&pool, &c, d.compra.id, "Venía equivocada", None).await,
        Err(Error::Pin(FalloPin::Requerido))
    ));
    assert!(compras::anular(&pool, &c, d.compra.id, "  ", Some("4821"))
        .await
        .is_err());
    let anulada = compras::anular(&pool, &c, d.compra.id, "Venía equivocada", Some("4821"))
        .await
        .unwrap();
    assert!(anulada.compra.anulada_en.is_some());
    assert_eq!(producto(&pool, "L").await.stock_mil, -3000);
    // El dinero volvió a la caja.
    let r = turnos::resumen(&pool, t.id).await.unwrap();
    assert_eq!((r.efectivo.ingresos, r.efectivo.esperado), (8000, 20_000));
    assert!(matches!(
        compras::anular(&pool, &c, d.compra.id, "Otra vez", Some("4821")).await,
        Err(Error::Validacion(m)) if m.contains("ya está anulada")
    ));
    // El movimiento que compensa es 'compra' negativo con la misma compra.
    let compensa: (i64, Option<i64>) = sqlx::query_as(
        "SELECT cantidad_mil, compra_id FROM movimientos_stock ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(compensa, (-10_000, Some(d.compra.id)));
}

#[tokio::test]
async fn el_detalle_advierte_si_anular_deja_stock_negativo() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let leche = comun::crear(&pool, "L", "Leche", 1190, 0).await;
    let d = compras::registrar(
        &pool,
        &ControlPin::new(),
        &compra(None, &[(leche.id, 5000, 800)], false),
        None,
    )
    .await
    .unwrap();
    assert!(d.quedarian_negativos.is_empty());
    sqlx::query("UPDATE productos SET stock_mil = 2000 WHERE id = ?")
        .bind(leche.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        compras::detalle(&pool, d.compra.id)
            .await
            .unwrap()
            .quedarian_negativos,
        ["Leche"]
    );
}

/// Producto creado desde Registrar compra: el formulario no tiene stock inicial (va en 0) y la
/// compra suma lo que llegó. Queda exactamente lo comprado, no el doble.
#[tokio::test]
async fn producto_nuevo_desde_la_compra_queda_con_lo_comprado() {
    let pool = db::abrir_en_memoria().await.unwrap();
    // Lo que envía FormularioProducto con origen="compra": stock inicial 0 y el costo escrito.
    let mut d = comun::datos(Some("7801234567890"), "Galletas de agua", 990, 0);
    d.costo = Some(620);
    let nuevo = productos::crear(&pool, &d).await.unwrap();
    assert_eq!(nuevo.stock_mil, 0);

    compras::registrar(
        &pool,
        &ControlPin::new(),
        &compra(None, &[(nuevo.id, 24_000, 620)], false),
        None,
    )
    .await
    .unwrap();
    let despues = producto(&pool, "7801234567890").await;
    assert_eq!(despues.stock_mil, 24_000);
    assert_eq!(despues.costo, Some(620));
}

/// La regla del servicio no cambió: un stock inicial se suma a lo que trae la compra. Por eso el
/// formulario no lo pide cuando el producto se crea desde una compra (era el stock fantasma).
#[tokio::test]
async fn un_stock_inicial_se_suma_a_la_compra() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let nuevo = comun::crear(&pool, "G", "Galletas de agua", 990, 10).await;
    compras::registrar(
        &pool,
        &ControlPin::new(),
        &compra(None, &[(nuevo.id, 24_000, 620)], false),
        None,
    )
    .await
    .unwrap();
    assert_eq!(producto(&pool, "G").await.stock_mil, 34_000);
}
