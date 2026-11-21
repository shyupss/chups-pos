mod comun;

use pos_core::boleta;
use pos_core::repositorios::productos::Unidad;
use pos_core::servicios::config::{self, Config, FormatoBalanza, TablaCaracteres};
use pos_core::servicios::negocio::{self, DatosNegocio};
use pos_core::servicios::ventas::{self, ItemPedido, MedioPago, NuevaVenta};
use pos_core::{db, Error};

async fn negocio_listo(pool: &sqlx::SqlitePool) {
    negocio::guardar(
        pool,
        &DatosNegocio {
            nombre: "Almacén Don Pepe".into(),
            rut: Some("12345678-5".into()),
            direccion: None,
            telefono: None,
            mensaje_pie: "Gracias".into(),
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn arma_la_boleta_desde_la_venta_guardada() {
    let pool = db::abrir_en_memoria().await.unwrap();
    negocio_listo(&pool).await;
    let pan = comun::crear_kg(&pool, "P", "Pan corriente", 2300, 10_000).await;
    let bebida = comun::crear(&pool, "B", "Bebida", 1500, 10).await;
    let v = ventas::confirmar(
        &pool,
        &NuevaVenta {
            items: vec![
                ItemPedido {
                    producto_id: pan.id,
                    cantidad_mil: 532,
                },
                ItemPedido {
                    producto_id: bebida.id,
                    cantidad_mil: 2000,
                },
            ],
            medio_pago: MedioPago::Efectivo,
            monto_recibido: Some(5000),
            descuento: 0,
            cliente_id: None,
        },
    )
    .await
    .unwrap();

    // Cambiar el producto después no cambia la boleta: la línea guardó nombre, unidad y precio.
    let mut d = comun::datos(Some("P"), "Pan (otro nombre)", 2600, 0);
    d.unidad = Unidad::Unidad;
    pos_core::servicios::productos::editar(&pool, pan.id, &d)
        .await
        .unwrap();

    let b = boleta::armar(&pool, v.id).await.unwrap();
    assert_eq!(b.negocio.nombre, "Almacén Don Pepe");
    assert_eq!(b.negocio.rut.as_deref(), Some("12.345.678-5"));
    assert_eq!(b.folio, 1);
    assert_eq!(b.lineas.len(), 2);
    assert_eq!(
        (
            b.lineas[0].nombre.as_str(),
            b.lineas[0].unidad,
            b.lineas[0].precio_unitario
        ),
        ("Pan corriente", Unidad::Kg, 2300)
    );
    assert_eq!((b.subtotal, b.redondeo, b.total), (4224, -4, 4220));
    assert_eq!((b.monto_recibido, b.vuelto), (Some(5000), Some(780)));
    assert_eq!(b.cantidad_productos(), 3);
    assert!(b.leyenda && b.codigo_barras);
    // dd-mm-aaaa hh:mm en la hora del equipo.
    assert_eq!(b.fecha.len(), 16, "{}", b.fecha);
    assert_eq!(
        (&b.fecha[2..3], &b.fecha[5..6], &b.fecha[13..14]),
        ("-", "-", ":")
    );

    let mut c = config::obtener(&pool).await.unwrap();
    c.boleta_leyenda = false;
    c.boleta_codigo = false;
    config::guardar(&pool, &c).await.unwrap();
    let b = boleta::armar(&pool, v.id).await.unwrap();
    assert!(!b.leyenda && !b.codigo_barras);

    assert_eq!(boleta::ultima_venta(&pool).await.unwrap(), Some(v.id));
}

#[tokio::test]
async fn sin_nombre_de_negocio_no_se_arma() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let p = comun::crear(&pool, "B", "Bebida", 1500, 10).await;
    let v = ventas::confirmar(
        &pool,
        &NuevaVenta {
            items: vec![ItemPedido {
                producto_id: p.id,
                cantidad_mil: 1000,
            }],
            medio_pago: MedioPago::Debito,
            monto_recibido: None,
            descuento: 0,
            cliente_id: None,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        boleta::armar(&pool, v.id).await,
        Err(Error::Validacion(_))
    ));
    negocio_listo(&pool).await;
    assert!(matches!(
        boleta::armar(&pool, 999).await,
        Err(Error::NoEncontrado(_))
    ));
}

#[tokio::test]
async fn configuracion_tipada_con_valores_por_defecto() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let c = config::obtener(&pool).await.unwrap();
    assert_eq!(c, Config::default());
    assert_eq!(c.columnas(), 48);
    assert_eq!(c.prefijos_balanza(), (20..=29).collect::<Vec<u8>>());

    let mut nueva = c.clone();
    nueva.impresora = Some("  POS-80  ".into());
    nueva.ancho_papel_mm = 58;
    nueva.tabla_caracteres = TablaCaracteres::Wpc1252;
    nueva.copias = 2;
    nueva.cajon_pin = 5;
    nueva.balanza_prefijos = " 20 - 22 , 25 ".into();
    nueva.balanza_formato = FormatoBalanza::Precio;
    let guardada = config::guardar(&pool, &nueva).await.unwrap();
    assert_eq!(guardada.impresora.as_deref(), Some("POS-80"));
    assert_eq!(guardada.columnas(), 32);
    assert_eq!(guardada.balanza_prefijos, "20-22,25");
    assert_eq!(guardada.prefijos_balanza(), [20, 21, 22, 25]);
    assert_eq!(config::obtener(&pool).await.unwrap(), guardada);

    for malo in [
        Config {
            ancho_papel_mm: 60,
            ..guardada.clone()
        },
        Config {
            copias: 3,
            ..guardada.clone()
        },
        Config {
            cajon_pin: 3,
            ..guardada.clone()
        },
        Config {
            balanza_largo_codigo: 7,
            ..guardada.clone()
        },
        Config {
            balanza_prefijos: "2".into(),
            ..guardada.clone()
        },
        Config {
            balanza_prefijos: "29-20".into(),
            ..guardada.clone()
        },
        Config {
            balanza_prefijos: "10-19".into(),
            ..guardada.clone()
        },
    ] {
        assert!(
            matches!(
                config::guardar(&pool, &malo).await,
                Err(Error::Validacion(_))
            ),
            "{malo:?}"
        );
    }

    // Un valor raro escrito a mano en la tabla se lee como el valor por defecto.
    sqlx::query("UPDATE config SET valor = 'xyz' WHERE clave = 'tabla_caracteres'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        config::obtener(&pool).await.unwrap().tabla_caracteres,
        TablaCaracteres::Cp858
    );
}

#[tokio::test]
async fn trabajos_de_impresion_segun_la_configuracion() {
    use pos_core::boleta::escpos;
    use pos_core::servicios::impresion;

    let pool = db::abrir_en_memoria().await.unwrap();
    negocio_listo(&pool).await;
    let p = comun::crear(&pool, "B", "Bebida", 1500, 10).await;
    let v = ventas::confirmar(
        &pool,
        &NuevaVenta {
            items: vec![ItemPedido {
                producto_id: p.id,
                cantidad_mil: 1000,
            }],
            medio_pago: MedioPago::Debito,
            monto_recibido: None,
            descuento: 0,
            cliente_id: None,
        },
    )
    .await
    .unwrap();

    // Sin impresora elegida: error claro que lleva a Configuración.
    let sin = impresion::trabajo_boleta(&pool, v.id, None).await;
    assert!(matches!(sin, Err(Error::Validacion(_))), "{sin:?}");

    let mut c = config::obtener(&pool).await.unwrap();
    c.impresora = Some("Archivo (pruebas)".into());
    c.copias = 2;
    c.cajon_pin = 5;
    config::guardar(&pool, &c).await.unwrap();

    let t = impresion::trabajo_boleta(&pool, v.id, None).await.unwrap();
    assert_eq!(
        (t.impresora.as_str(), t.documento.as_str()),
        ("Archivo (pruebas)", "boleta-000001")
    );
    let cortes = t.bytes.windows(4).filter(|w| *w == escpos::CORTE).count();
    assert_eq!(cortes, 2, "dos copias");
    let una = impresion::trabajo_boleta(&pool, v.id, Some(1))
        .await
        .unwrap();
    assert_eq!(una.bytes.len() * 2, t.bytes.len());

    let cajon = impresion::trabajo_cajon(&pool).await.unwrap();
    assert_eq!(cajon.bytes, escpos::pulso_cajon(5, 100));
    let prueba = impresion::trabajo_prueba(&pool).await.unwrap();
    assert!(prueba.bytes.starts_with(&escpos::INICIALIZAR));
}
