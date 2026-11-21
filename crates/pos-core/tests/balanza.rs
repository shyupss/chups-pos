mod comun;

use pos_core::balanza::{self, CodigoBalanza, ReglasBalanza, ValorBalanza};
use pos_core::servicios::balanza as srv;
use pos_core::servicios::config::{self, FormatoBalanza};
use pos_core::{db, Error};

fn reglas(formato: FormatoBalanza, largo_codigo: usize) -> ReglasBalanza {
    ReglasBalanza {
        prefijos: (20..=29).collect(),
        formato,
        largo_codigo,
    }
}

/// Agrega el dígito verificador correcto a 12 dígitos.
fn con_digito(doce: &str) -> String {
    let d: Vec<u8> = doce.bytes().map(|b| b - b'0').collect();
    format!("{doce}{}", balanza::digito_verificador(&d))
}

#[test]
fn etiqueta_con_peso_en_gramos() {
    let codigo = "2012345005327"; // 20 · 12345 · 00532 · 7
    assert_eq!(
        balanza::analizar(codigo, &reglas(FormatoBalanza::Peso, 5)).unwrap(),
        Some(CodigoBalanza {
            codigo_producto: "12345".into(),
            valor: ValorBalanza::Gramos(532),
        })
    );
}

#[test]
fn etiqueta_con_precio_y_largos_de_codigo() {
    let codigo = con_digito("280012301224"); // 28 · 00123 · 01224
    assert_eq!(
        balanza::analizar(&codigo, &reglas(FormatoBalanza::Precio, 5)).unwrap(),
        Some(CodigoBalanza {
            codigo_producto: "00123".into(),
            valor: ValorBalanza::Pesos(1224),
        })
    );
    // Con 4 dígitos de código el valor tiene 6; con 6, tiene 4.
    let a = balanza::analizar(&codigo, &reglas(FormatoBalanza::Precio, 4))
        .unwrap()
        .unwrap();
    assert_eq!(
        (a.codigo_producto.as_str(), a.valor),
        ("0012", ValorBalanza::Pesos(301_224))
    );
    let b = balanza::analizar(&codigo, &reglas(FormatoBalanza::Peso, 6))
        .unwrap()
        .unwrap();
    assert_eq!(
        (b.codigo_producto.as_str(), b.valor),
        ("001230", ValorBalanza::Gramos(1224))
    );

    // $1.224 de un producto a $2.300/kg son 532 g (531,9 redondeado).
    assert_eq!(balanza::gramos_desde_precio(1224, 2300), Some(532));
    assert_eq!(balanza::gramos_desde_precio(1224, 0), None);
}

#[test]
fn digito_verificador_incorrecto_es_error() {
    let r = balanza::analizar("2012345005321", &reglas(FormatoBalanza::Peso, 5));
    assert!(matches!(r, Err(Error::Validacion(_))), "{r:?}");
}

#[test]
fn lo_que_no_es_etiqueta_de_balanza_se_ignora() {
    let r = reglas(FormatoBalanza::Peso, 5);
    for codigo in [
        "7801610001196",  // EAN-13 normal: prefijo 78
        "201234500532",   // 12 dígitos
        "20123450053277", // 14 dígitos
        "20123A5005327",  // letras
        "",
    ] {
        assert_eq!(balanza::analizar(codigo, &r).unwrap(), None, "{codigo:?}");
    }
    // Prefijo 23 fuera de la lista configurada.
    let solo_20 = ReglasBalanza {
        prefijos: vec![20],
        ..r
    };
    assert_eq!(
        balanza::analizar(&con_digito("230000100500"), &solo_20).unwrap(),
        None
    );
}

#[tokio::test]
async fn la_caja_lee_la_etiqueta_y_encuentra_el_producto() {
    let pool = db::abrir_en_memoria().await.unwrap();
    let pan = comun::crear_kg(&pool, "123", "Pan corriente", 2300, 0).await;
    comun::crear(&pool, "456", "Bebida", 1500, 0).await;
    let codigo = con_digito("200012300532"); // producto 00123 → "123" sin ceros

    // Desactivados por defecto: no se interpreta.
    assert_eq!(srv::leer(&pool, &codigo).await.unwrap(), None);

    let mut c = config::obtener(&pool).await.unwrap();
    c.balanza_activa = true;
    config::guardar(&pool, &c).await.unwrap();
    let lectura = srv::leer(&pool, &codigo).await.unwrap().unwrap();
    assert_eq!((lectura.producto.id, lectura.cantidad_mil), (pan.id, 532));

    // Por precio: $1.224 a $2.300/kg = 532 g.
    c.balanza_formato = FormatoBalanza::Precio;
    config::guardar(&pool, &c).await.unwrap();
    let por_precio = con_digito("200012301224");
    assert_eq!(
        srv::leer(&pool, &por_precio)
            .await
            .unwrap()
            .unwrap()
            .cantidad_mil,
        532
    );

    // Producto por unidad, producto que no existe y peso cero: errores claros.
    c.balanza_formato = FormatoBalanza::Peso;
    config::guardar(&pool, &c).await.unwrap();
    let unidad = srv::leer(&pool, &con_digito("200045600100")).await;
    assert!(matches!(unidad, Err(Error::Validacion(_))), "{unidad:?}");
    let no_existe = srv::leer(&pool, &con_digito("209999900100")).await;
    assert!(
        matches!(no_existe, Err(Error::NoEncontrado(_))),
        "{no_existe:?}"
    );
    let cero = srv::leer(&pool, &con_digito("200012300000")).await;
    assert!(matches!(cero, Err(Error::Validacion(_))), "{cero:?}");
    // Un EAN normal no es de balanza.
    assert_eq!(srv::leer(&pool, "7801610001196").await.unwrap(), None);
}
