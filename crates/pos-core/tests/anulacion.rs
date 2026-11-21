// Anular una venta: stock, reembolso, turno, PIN y estados que no se anulan.

mod comun;

use pos_core::servicios::anulacion;
use pos_core::servicios::historial::{self, EstadoVenta};
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

async fn anular(
    pool: &SqlitePool,
    venta_id: i64,
    medio: Option<MedioPago>,
) -> Result<historial::DetalleVenta, Error> {
    anulacion::anular(
        pool,
        &ControlPin::new(),
        venta_id,
        "Cliente se arrepintió",
        medio,
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
async fn repone_el_stock_por_unidad_y_por_kg() {
    let pool = comun::con_caja().await;
    let pan = comun::crear_kg(&pool, "P", "Pan", 2300, 10_000).await;
    let bebida = comun::crear(&pool, "B", "Bebida", 1500, 10).await;
    let v = vender(
        &pool,
        &[(pan.id, 532), (bebida.id, 3000)],
        MedioPago::Efectivo,
    )
    .await;
    assert_eq!(
        (stock(&pool, "P").await, stock(&pool, "B").await),
        (9468, 7000)
    );

    let d = anular(&pool, v.id, Some(MedioPago::Efectivo))
        .await
        .unwrap();
    assert_eq!(d.estado, EstadoVenta::Anulada);
    assert_eq!(
        (stock(&pool, "P").await, stock(&pool, "B").await),
        (10_000, 10_000)
    );
    let a = d.anulacion.unwrap();
    assert_eq!(a.motivo, "Cliente se arrepintió");
    assert_eq!(a.medio_reembolso, Some(MedioPago::Efectivo));
    assert_eq!(a.reembolso, Some(v.total));
    assert!(a.turno_id.is_some());

    // Un movimiento 'devolucion' por línea, en milésimas, ligado a la venta.
    let movimientos: Vec<(String, i64)> = sqlx::query_as(
        "SELECT tipo, cantidad_mil FROM movimientos_stock WHERE venta_id = ? AND cantidad_mil > 0
         ORDER BY id",
    )
    .bind(v.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        movimientos,
        [
            ("devolucion".to_owned(), 532),
            ("devolucion".to_owned(), 3000)
        ]
    );

    // La boleta de la venta ahora dice ANULADA y sale el comprobante de la anulación.
    sqlx::query("UPDATE negocio SET nombre = 'Almacén'")
        .execute(&pool)
        .await
        .unwrap();
    let b = pos_core::boleta::armar(&pool, v.id).await.unwrap();
    assert!(b.anulada.is_some());
    let c = pos_core::boleta::anulacion::armar(&pool, v.id)
        .await
        .unwrap();
    assert_eq!(
        (c.folio, c.reembolso, c.lineas.len()),
        (v.folio, Some(v.total), 2)
    );
    use pos_core::boleta::Documento;
    assert_eq!(c.nombre(), format!("anulacion-{:06}", v.folio));
}

#[tokio::test]
async fn no_se_anula_dos_veces_y_exige_motivo_y_medio() {
    let pool = comun::con_caja().await;
    let p = comun::crear(&pool, "B", "Bebida", 1500, 10).await;
    let v = vender(&pool, &[(p.id, 1000)], MedioPago::Debito).await;

    assert!(matches!(
        anulacion::anular(&pool, &ControlPin::new(), v.id, "  ", Some(MedioPago::Debito), None).await,
        Err(Error::Validacion(m)) if m.contains("motivo")
    ));
    assert!(matches!(
        anular(&pool, v.id, None).await,
        Err(Error::Validacion(m)) if m.contains("devuelve el dinero")
    ));
    assert!(anular(&pool, v.id, Some(MedioPago::Fiado)).await.is_err());

    anular(&pool, v.id, Some(MedioPago::Debito)).await.unwrap();
    assert!(matches!(
        anular(&pool, v.id, Some(MedioPago::Debito)).await,
        Err(Error::Validacion(m)) if m == "La venta ya está anulada"
    ));
    // El stock se repuso una sola vez.
    assert_eq!(stock(&pool, "B").await, 10_000);
    assert!(anular(&pool, 999, Some(MedioPago::Debito)).await.is_err());
}

#[tokio::test]
async fn el_reembolso_en_efectivo_baja_el_esperado_del_turno() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let pan = comun::crear_kg(&pool, "P", "Pan", 2300, 10_000).await;
    let bebida = comun::crear(&pool, "B", "Bebida", 1500, 10).await;
    let t = turnos::abrir(&pool, 10_000).await.unwrap();

    let efectivo = vender(&pool, &[(bebida.id, 2000)], MedioPago::Efectivo).await;
    // Débito de 0,532 kg = $1.224: anulada en efectivo devuelve $1.220 (a la decena).
    let debito = vender(&pool, &[(pan.id, 532)], MedioPago::Debito).await;
    let credito = vender(&pool, &[(bebida.id, 1000)], MedioPago::Credito).await;
    let antes = turnos::resumen(&pool, t.id)
        .await
        .unwrap()
        .efectivo
        .esperado;
    assert_eq!(antes, 13_000);

    anular(&pool, efectivo.id, Some(MedioPago::Efectivo))
        .await
        .unwrap();
    let d = anular(&pool, debito.id, Some(MedioPago::Efectivo))
        .await
        .unwrap();
    assert_eq!(d.anulacion.unwrap().reembolso, Some(1220));
    // Con tarjeta no toca la caja.
    anular(&pool, credito.id, Some(MedioPago::Credito))
        .await
        .unwrap();

    let r = turnos::resumen(&pool, t.id).await.unwrap();
    assert_eq!(r.efectivo.reembolsos, 3000 + 1220);
    assert_eq!(r.efectivo.esperado, 13_000 - 3000 - 1220);
    // Las ventas siguen contando en el turno (el dinero entró) y las anulaciones aparte.
    assert_eq!(r.cantidad_ventas, 3);
    let anulaciones: Vec<(MedioPago, i64, i64, i64)> = r
        .anulaciones
        .iter()
        .map(|t| (t.medio, t.cantidad, t.monto, t.redondeo))
        .collect();
    assert_eq!(
        anulaciones,
        [
            (MedioPago::Efectivo, 2, 4220, -4),
            (MedioPago::Credito, 1, 1500, 0)
        ]
    );
    assert_eq!(r.redondeo_reembolsos, -4);
}

#[tokio::test]
async fn una_venta_de_un_turno_cerrado_se_reembolsa_del_turno_abierto() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let control = ControlPin::new();
    let p = comun::crear(&pool, "B", "Bebida", 1500, 10).await;
    let primero = turnos::abrir(&pool, 0).await.unwrap();
    let v = vender(&pool, &[(p.id, 1000)], MedioPago::Efectivo).await;
    let cerrado = turnos::cerrar(&pool, &control, 1500, None, None, None)
        .await
        .unwrap();

    // Sin caja abierta no se devuelve efectivo, pero con débito sí (y queda sin turno).
    assert!(matches!(
        anular(&pool, v.id, Some(MedioPago::Efectivo)).await,
        Err(Error::Validacion(m)) if m == "Abre la caja para devolver efectivo"
    ));
    let segundo = turnos::abrir(&pool, 5000).await.unwrap();
    let d = anular(&pool, v.id, Some(MedioPago::Efectivo))
        .await
        .unwrap();
    assert_eq!(d.anulacion.unwrap().turno_id, Some(segundo.id));

    assert_eq!(turnos::resumen(&pool, primero.id).await.unwrap(), cerrado);
    assert_eq!(
        turnos::resumen(&pool, segundo.id)
            .await
            .unwrap()
            .efectivo
            .esperado,
        5000 - 1500
    );
}

#[tokio::test]
async fn la_anulacion_sin_caja_con_tarjeta_queda_sin_turno() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let p = comun::crear(&pool, "B", "Bebida", 1500, 10).await;
    turnos::abrir(&pool, 0).await.unwrap();
    let v = vender(&pool, &[(p.id, 1000)], MedioPago::Transferencia).await;
    turnos::cerrar(&pool, &ControlPin::new(), 0, None, None, None)
        .await
        .unwrap();
    let d = anular(&pool, v.id, Some(MedioPago::Transferencia))
        .await
        .unwrap();
    assert_eq!(d.anulacion.unwrap().turno_id, None);
}

#[tokio::test]
async fn la_fiada_anulada_deja_de_ser_deuda_y_con_abonos_se_reembolsa() {
    let pool = comun::con_caja().await;
    let p = comun::crear(&pool, "B", "Bebida", 1500, 10).await;
    let a = vender(&pool, &[(p.id, 2000)], MedioPago::Fiado).await;
    let b = vender(&pool, &[(p.id, 1000)], MedioPago::Fiado).await;

    // Sin abonos no hay nada que devolver: el medio elegido no se usa.
    let d = anular(&pool, a.id, Some(MedioPago::Efectivo))
        .await
        .unwrap();
    let anulacion = d.anulacion.unwrap();
    assert_eq!(
        (anulacion.medio_reembolso, anulacion.reembolso),
        (None, None)
    );
    let deudas: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ventas WHERE estado = 'fiada'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(deudas, 1);

    // Con $500 abonados y sin otras deudas, hay que elegir cómo devolverlos.
    sqlx::query("INSERT INTO abonos (venta_id, monto, medio_pago) VALUES (?, 500, 'efectivo')")
        .bind(b.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        anular(&pool, b.id, None).await,
        Err(Error::Validacion(m)) if m.contains("$500")
    ));
    let d = anular(&pool, b.id, Some(MedioPago::Efectivo))
        .await
        .unwrap();
    let anulacion = d.anulacion.unwrap();
    assert_eq!(
        (anulacion.medio_reembolso, anulacion.reembolso),
        (Some(MedioPago::Efectivo), Some(500))
    );
}

#[tokio::test]
async fn anular_pide_pin_cuando_esta_protegido() {
    let pool = comun::con_caja().await;
    let p = comun::crear(&pool, "B", "Bebida", 1500, 10).await;
    let v = vender(&pool, &[(p.id, 1000)], MedioPago::Efectivo).await;
    seguridad::crear_pin(&pool, "4821").await.unwrap();
    let control = ControlPin::new();
    let intento = |pin: Option<&'static str>| {
        let (pool, control) = (&pool, &control);
        async move {
            anulacion::anular(
                pool,
                control,
                v.id,
                "Error de cobro",
                Some(MedioPago::Efectivo),
                pin,
            )
            .await
        }
    };

    assert!(matches!(
        intento(None).await,
        Err(Error::Pin(FalloPin::Requerido))
    ));
    assert!(matches!(
        intento(Some("0000")).await,
        Err(Error::Pin(FalloPin::Incorrecto { .. }))
    ));
    // Nada cambió con los intentos fallidos.
    assert_eq!(
        historial::detalle(&pool, v.id).await.unwrap().estado,
        EstadoVenta::Pagada
    );
    assert_eq!(stock(&pool, "B").await, 9000);
    intento(Some("4821")).await.unwrap();
    assert_eq!(stock(&pool, "B").await, 10_000);
}
