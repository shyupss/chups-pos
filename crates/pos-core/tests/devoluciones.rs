// Devoluciones parciales: por unidad y por kg, límites, montos, redondeo, merma y estado.

mod comun;

use pos_core::servicios::anulacion;
use pos_core::servicios::devoluciones::{self, DevolucionRegistrada, ItemDevuelto};
use pos_core::servicios::historial::{self, EstadoFiltro, EstadoVenta, FiltrosVentas};
use pos_core::servicios::productos;
use pos_core::servicios::seguridad::{self, ControlPin};
use pos_core::servicios::turnos;
use pos_core::servicios::ventas::{self, ItemPedido, MedioPago, NuevaVenta, VentaConfirmada};
use pos_core::{db, Error, FalloPin};
use sqlx::SqlitePool;

async fn vender(pool: &SqlitePool, items: &[(i64, i64)], medio: MedioPago) -> VentaConfirmada {
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
            cliente_id,
        },
    )
    .await
    .unwrap()
}

/// Ids de las líneas vendidas, en el orden de la venta.
async fn lineas(pool: &SqlitePool, venta_id: i64) -> Vec<i64> {
    historial::detalle(pool, venta_id)
        .await
        .unwrap()
        .lineas
        .iter()
        .map(|l| l.id)
        .collect()
}

fn item(venta_item_id: i64, cantidad_mil: i64) -> ItemDevuelto {
    ItemDevuelto {
        venta_item_id,
        cantidad_mil,
        vuelve_a_stock: true,
    }
}

async fn devolver(
    pool: &SqlitePool,
    venta_id: i64,
    items: &[ItemDevuelto],
    medio: Option<MedioPago>,
) -> Result<DevolucionRegistrada, Error> {
    devoluciones::registrar(
        pool,
        &ControlPin::new(),
        venta_id,
        items,
        medio,
        "Venía mala",
        None,
    )
    .await
}

async fn stock(pool: &SqlitePool, codigo: &str) -> i64 {
    productos::buscar_por_codigo(pool, codigo)
        .await
        .unwrap()
        .unwrap()
        .stock_mil
}

#[tokio::test]
async fn devolucion_parcial_por_unidad_y_por_kg() {
    let pool = comun::con_caja().await;
    let leche = comun::crear(&pool, "L", "Leche", 1190, 10).await;
    let pan = comun::crear_kg(&pool, "P", "Pan", 2300, 10_000).await;
    let v = vender(&pool, &[(leche.id, 3000), (pan.id, 532)], MedioPago::Debito).await;
    let ids = lineas(&pool, v.id).await;

    // 1 leche de 3 ($3.570 / 3 = $1.190) y la mitad del pan ($1.224 / 2 = $612).
    let r = devolver(
        &pool,
        v.id,
        &[item(ids[0], 1000), item(ids[1], 266)],
        Some(MedioPago::Debito),
    )
    .await
    .unwrap();
    assert_eq!((r.monto, r.redondeo), (1190 + 612, 0));
    assert_eq!(r.venta.estado, EstadoVenta::ConDevolucion);
    assert_eq!(r.venta.monto_devuelto, 1802);
    assert_eq!(
        r.venta
            .lineas
            .iter()
            .map(|l| l.devuelto_mil)
            .collect::<Vec<_>>(),
        [1000, 266]
    );
    assert_eq!(
        (stock(&pool, "L").await, stock(&pool, "P").await),
        (8000, 9734)
    );

    let d = &r.venta.devoluciones[0];
    assert_eq!(d.medio_reembolso, Some(MedioPago::Debito));
    assert_eq!(d.motivo, "Venía mala");
    assert_eq!(
        d.lineas
            .iter()
            .map(|l| (l.cantidad_mil, l.monto))
            .collect::<Vec<_>>(),
        [(1000, 1190), (266, 612)]
    );

    // Por unidad, solo unidades enteras.
    assert!(matches!(
        devolver(&pool, v.id, &[item(ids[0], 1500)], Some(MedioPago::Debito)).await,
        Err(Error::Validacion(m)) if m.contains("unidades enteras")
    ));
}

#[tokio::test]
async fn no_se_devuelve_mas_de_lo_vendido_en_varias_devoluciones() {
    let pool = comun::con_caja().await;
    // 3 unidades por $1.000 en total: de a una se devuelven 333, 334 y 333.
    let p = comun::crear(&pool, "B", "Bebida", 0, 10).await;
    sqlx::query("UPDATE productos SET precio_venta = 1000 WHERE id = ?")
        .bind(p.id)
        .execute(&pool)
        .await
        .unwrap();
    let pan = comun::crear_kg(&pool, "P", "Pan", 1000, 10_000).await;
    let v = vender(
        &pool,
        &[(p.id, 3000), (pan.id, 1000)],
        MedioPago::Transferencia,
    )
    .await;
    let ids = lineas(&pool, v.id).await;
    let t = Some(MedioPago::Transferencia);

    devolver(&pool, v.id, &[item(ids[0], 2000)], t)
        .await
        .unwrap();
    assert!(matches!(
        devolver(&pool, v.id, &[item(ids[0], 2000)], t).await,
        Err(Error::Validacion(m)) if m.contains("solo quedan 1")
    ));
    devolver(&pool, v.id, &[item(ids[0], 1000)], t)
        .await
        .unwrap();
    assert!(matches!(
        devolver(&pool, v.id, &[item(ids[0], 1000)], t).await,
        Err(Error::Validacion(m)) if m.contains("ya se devolvió completo")
    ));
    // Repetida, de otra venta o en cero: no.
    assert!(
        devolver(&pool, v.id, &[item(ids[1], 100), item(ids[1], 100)], t)
            .await
            .is_err()
    );
    assert!(devolver(&pool, v.id, &[item(999, 100)], t).await.is_err());
    assert!(devolver(&pool, v.id, &[item(ids[1], 0)], t).await.is_err());
    assert!(devolver(&pool, v.id, &[], t).await.is_err());

    let d = historial::detalle(&pool, v.id).await.unwrap();
    // 3.000 de bebidas: lo devuelto de esa línea suma su subtotal.
    let devuelto_bebida: i64 = d
        .devoluciones
        .iter()
        .flat_map(|x| &x.lineas)
        .filter(|l| l.venta_item_id == ids[0])
        .map(|l| l.monto)
        .sum();
    assert_eq!(devuelto_bebida, 3000);
    assert_eq!(d.estado, EstadoVenta::ConDevolucion);
}

#[tokio::test]
async fn el_monto_es_proporcional_y_en_efectivo_se_redondea() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let queso = comun::crear_kg(&pool, "Q", "Queso", 1000, 10_000).await;
    let t = turnos::abrir(&pool, 10_000).await.unwrap();
    // 1 kg = $1.000 en débito; se devuelve en tres partes en efectivo.
    let v = vender(&pool, &[(queso.id, 1000)], MedioPago::Debito).await;
    let id = lineas(&pool, v.id).await[0];
    let e = Some(MedioPago::Efectivo);

    let a = devolver(&pool, v.id, &[item(id, 333)], e).await.unwrap();
    assert_eq!((a.monto, a.redondeo), (330, -3));
    let b = devolver(&pool, v.id, &[item(id, 333)], e).await.unwrap();
    assert_eq!((b.monto, b.redondeo), (330, -3));
    let c = devolver(&pool, v.id, &[item(id, 334)], e).await.unwrap();
    // 1.000 − 666 = 334 de la línea, que en efectivo son 330.
    assert_eq!((c.monto, c.redondeo), (330, -4));
    assert_eq!(c.venta.estado, EstadoVenta::Devuelta);
    assert_eq!(c.venta.monto_devuelto, 990);

    // Las tres salieron del efectivo del turno.
    let r = turnos::resumen(&pool, t.id).await.unwrap();
    assert_eq!(r.efectivo.reembolsos, 990);
    assert_eq!(r.efectivo.esperado, 10_000 - 990);
    assert_eq!(
        r.devoluciones
            .iter()
            .map(|t| (t.medio, t.cantidad, t.monto))
            .collect::<Vec<_>>(),
        [(MedioPago::Efectivo, 3, 990)]
    );
    assert_eq!(r.redondeo_reembolsos, -10);
}

#[tokio::test]
async fn la_merma_no_repone_stock_pero_queda_anotada() {
    let pool = comun::con_caja().await;
    let leche = comun::crear(&pool, "L", "Leche", 1190, 10).await;
    let v = vender(&pool, &[(leche.id, 2000)], MedioPago::Efectivo).await;
    let id = lineas(&pool, v.id).await[0];

    let r = devolver(
        &pool,
        v.id,
        &[ItemDevuelto {
            venta_item_id: id,
            cantidad_mil: 1000,
            vuelve_a_stock: false,
        }],
        Some(MedioPago::Efectivo),
    )
    .await
    .unwrap();
    assert_eq!(stock(&pool, "L").await, 8000);
    assert!(!r.venta.devoluciones[0].lineas[0].vuelve_a_stock);
    let (tipo, cantidad, nota): (String, i64, String) = sqlx::query_as(
        "SELECT tipo, cantidad_mil, nota FROM movimientos_stock ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((tipo.as_str(), cantidad), ("merma", 0));
    assert!(nota.contains("no vuelve al stock"), "{nota}");
}

#[tokio::test]
async fn al_devolver_todo_la_venta_queda_devuelta() {
    let pool = comun::con_caja().await;
    let leche = comun::crear(&pool, "L", "Leche", 1190, 10).await;
    let pan = comun::crear_kg(&pool, "P", "Pan", 2300, 10_000).await;
    let v = vender(
        &pool,
        &[(leche.id, 1000), (pan.id, 532)],
        MedioPago::Credito,
    )
    .await;
    let ids = lineas(&pool, v.id).await;
    let c = Some(MedioPago::Credito);

    let r = devolver(&pool, v.id, &[item(ids[0], 1000)], c)
        .await
        .unwrap();
    assert_eq!(r.venta.estado, EstadoVenta::ConDevolucion);
    // Con devoluciones ya no se anula: se devuelve el resto.
    assert!(matches!(
        anulacion::anular(&pool, &ControlPin::new(), v.id, "x", c, None).await,
        Err(Error::Validacion(m)) if m.contains("devoluciones")
    ));
    let r = devolver(&pool, v.id, &[item(ids[1], 532)], c)
        .await
        .unwrap();
    assert_eq!(r.venta.estado, EstadoVenta::Devuelta);
    assert_eq!(r.venta.monto_devuelto, v.total);
    assert!(matches!(
        devolver(&pool, v.id, &[item(ids[1], 1)], c).await,
        Err(Error::Validacion(m)) if m == "Ya se devolvió todo lo vendido"
    ));

    // En el historial: con devolución y fuera del total del filtro.
    let f = FiltrosVentas {
        estado: Some(EstadoFiltro::ConDevolucion),
        ..Default::default()
    };
    let lista = historial::listar(&pool, &f, 10, 0).await.unwrap();
    assert_eq!(lista.ventas[0].estado, EstadoVenta::Devuelta);
    assert_eq!(lista.totales.unwrap().total, 0);
}

#[tokio::test]
async fn una_fiada_devuelta_baja_la_deuda_sin_reembolso() {
    let pool = comun::con_caja().await;
    let leche = comun::crear(&pool, "L", "Leche", 1190, 10).await;
    let v = vender(&pool, &[(leche.id, 2000)], MedioPago::Fiado).await;
    let id = lineas(&pool, v.id).await[0];

    assert!(
        devolver(&pool, v.id, &[item(id, 1000)], Some(MedioPago::Efectivo))
            .await
            .is_err()
    );
    let r = devolver(&pool, v.id, &[item(id, 1000)], None)
        .await
        .unwrap();
    assert_eq!((r.monto, r.redondeo), (1190, 0));
    assert_eq!(r.venta.devoluciones[0].medio_reembolso, None);
    assert_eq!(r.venta.monto_devuelto, 1190);
    // No tocó el efectivo del turno.
    let turno = turnos::estado_caja(&pool).await.unwrap().turno.unwrap();
    assert_eq!(
        turnos::resumen(&pool, turno.id)
            .await
            .unwrap()
            .efectivo
            .reembolsos,
        0
    );
}

#[tokio::test]
async fn sin_caja_no_se_devuelve_efectivo() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let leche = comun::crear(&pool, "L", "Leche", 1190, 10).await;
    turnos::abrir(&pool, 0).await.unwrap();
    let v = vender(&pool, &[(leche.id, 2000)], MedioPago::Debito).await;
    turnos::cerrar(&pool, &ControlPin::new(), 0, None, None, None)
        .await
        .unwrap();
    let id = lineas(&pool, v.id).await[0];
    assert!(matches!(
        devolver(&pool, v.id, &[item(id, 1000)], Some(MedioPago::Efectivo)).await,
        Err(Error::Validacion(m)) if m == "Abre la caja para devolver efectivo"
    ));
    let r = devolver(&pool, v.id, &[item(id, 1000)], Some(MedioPago::Debito))
        .await
        .unwrap();
    assert_eq!(r.venta.devoluciones[0].turno_id, None);
}

#[tokio::test]
async fn devolver_pide_pin_cuando_esta_protegido() {
    let pool = comun::con_caja().await;
    let leche = comun::crear(&pool, "L", "Leche", 1190, 10).await;
    let v = vender(&pool, &[(leche.id, 2000)], MedioPago::Efectivo).await;
    let id = lineas(&pool, v.id).await[0];
    seguridad::crear_pin(&pool, "4821").await.unwrap();
    let control = ControlPin::new();
    let items = [item(id, 1000)];
    let e = Some(MedioPago::Efectivo);

    assert!(matches!(
        devoluciones::registrar(&pool, &control, v.id, &items, e, "Mala", None).await,
        Err(Error::Pin(FalloPin::Requerido))
    ));
    assert!(matches!(
        devoluciones::registrar(&pool, &control, v.id, &items, e, "Mala", Some("1111")).await,
        Err(Error::Pin(FalloPin::Incorrecto { .. }))
    ));
    assert_eq!(stock(&pool, "L").await, 8000);
    devoluciones::registrar(&pool, &control, v.id, &items, e, "Mala", Some("4821"))
        .await
        .unwrap();
    assert_eq!(stock(&pool, "L").await, 9000);
}

#[tokio::test]
async fn la_nota_de_devolucion_sale_en_la_termica() {
    use pos_core::servicios::config::{self, Config};
    use pos_core::servicios::impresion;

    let pool = comun::con_caja().await;
    let leche = comun::crear(&pool, "L", "Leche", 1190, 10).await;
    let v = vender(&pool, &[(leche.id, 2000)], MedioPago::Efectivo).await;
    let id = lineas(&pool, v.id).await[0];
    let r = devolver(&pool, v.id, &[item(id, 1000)], Some(MedioPago::Efectivo))
        .await
        .unwrap();
    sqlx::query("UPDATE negocio SET nombre = 'Almacén'")
        .execute(&pool)
        .await
        .unwrap();
    let c = Config {
        impresora: Some("Archivo (pruebas)".into()),
        ..Config::default()
    };
    config::guardar(&pool, &ControlPin::new(), None, &c)
        .await
        .unwrap();
    let t = impresion::trabajo_devolucion(&pool, r.devolucion_id)
        .await
        .unwrap();
    assert_eq!(t.documento, format!("devolucion-{:06}", r.devolucion_id));
    let contiene = |texto: &[u8]| t.bytes.windows(texto.len()).any(|w| w == texto);
    // "Venta N° 000001" en CP858, donde "°" es el byte 0xF8.
    let mut folio = b"Venta N\xf8 ".to_vec();
    folio.extend(format!("{:06}", v.folio).bytes());
    assert!(contiene(&folio));
    assert!(contiene(b"$1.190"));
}
