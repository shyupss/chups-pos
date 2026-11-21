use pos_core::servicios::negocio::{self, DatosNegocio};
use pos_core::servicios::seguridad::ControlPin;
use pos_core::{db, rut, Error};

#[test]
fn rut_valido_con_y_sin_puntos() {
    for entrada in ["12.345.678-5", "12345678-5", "123456785", " 12 345 678 5 "] {
        assert_eq!(
            rut::normalizar(entrada).unwrap(),
            "12.345.678-5",
            "{entrada}"
        );
    }
    assert_eq!(rut::normalizar("10.000.013-k").unwrap(), "10.000.013-K");
    assert_eq!(rut::normalizar("1-9").unwrap(), "1-9");
    assert_eq!(rut::normalizar("76.086.428-5").unwrap(), "76.086.428-5");
}

#[test]
fn rut_invalido() {
    for entrada in [
        "12.345.678-4", // dígito verificador incorrecto
        "12.345.678-K",
        "",
        "5",
        "abc",
        "12.345.678-X",
        "0-0",
        "1234567890123-5",
        "12.3a5.678-5",
    ] {
        assert!(
            matches!(rut::normalizar(entrada), Err(Error::Validacion(_))),
            "{entrada:?} debería ser inválido"
        );
    }
}

fn datos(nombre: &str, rut: Option<&str>) -> DatosNegocio {
    DatosNegocio {
        nombre: nombre.into(),
        rut: rut.map(str::to_owned),
        direccion: Some("  Av. Siempre Viva 742, Talca ".into()),
        telefono: Some("".into()),
        mensaje_pie: "¡Gracias por su compra!".into(),
    }
}

#[tokio::test]
async fn guarda_los_datos_normalizados() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let inicial = negocio::obtener(&pool).await.unwrap();
    assert_eq!(inicial.nombre, "");
    assert_eq!(inicial.mensaje_pie, "Gracias por su compra");

    let n = negocio::guardar(
        &pool,
        &ControlPin::new(),
        None,
        &datos("  Almacén Don Pepe ", Some("123456785")),
    )
    .await
    .unwrap();
    assert_eq!(n.nombre, "Almacén Don Pepe");
    assert_eq!(n.rut.as_deref(), Some("12.345.678-5"));
    assert_eq!(n.direccion.as_deref(), Some("Av. Siempre Viva 742, Talca"));
    assert_eq!(n.telefono, None, "vacío queda como sin teléfono");

    let sin_nombre = negocio::guardar(&pool, &ControlPin::new(), None, &datos("   ", None)).await;
    assert!(matches!(sin_nombre, Err(Error::Validacion(_))));
    let rut_malo = negocio::guardar(
        &pool,
        &ControlPin::new(),
        None,
        &datos("Otro", Some("12.345.678-0")),
    )
    .await;
    assert!(matches!(rut_malo, Err(Error::Validacion(_))));
    assert_eq!(
        negocio::obtener(&pool).await.unwrap().nombre,
        "Almacén Don Pepe",
        "un error no guarda nada"
    );
}

#[tokio::test]
async fn carga_y_quita_el_logo() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let imagen = image::RgbImage::from_fn(800, 300, |x, y| {
        if (x / 40 + y / 40) % 2 == 0 {
            image::Rgb([20, 20, 20])
        } else {
            image::Rgb([240, 240, 240])
        }
    });
    let mut jpg = std::io::Cursor::new(Vec::new());
    imagen.write_to(&mut jpg, image::ImageFormat::Jpeg).unwrap();

    let n = negocio::guardar_logo(&pool, &ControlPin::new(), None, jpg.get_ref())
        .await
        .unwrap();
    let png = n.logo_png.expect("hay logo");
    assert!(png.starts_with("iVBORw0KGgo"), "es un PNG en base64");

    let guardado: Vec<u8> = sqlx::query_scalar("SELECT logo FROM negocio WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let logo = pos_core::logo::decodificar(&guardado).unwrap();
    assert_eq!((logo.ancho, logo.alto), (384, 144));

    assert!(negocio::quitar_logo(&pool, &ControlPin::new(), None)
        .await
        .unwrap()
        .logo_png
        .is_none());
    assert!(
        negocio::guardar_logo(&pool, &ControlPin::new(), None, b"esto no es una imagen")
            .await
            .is_err()
    );
}
