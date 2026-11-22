// Emitir, verificar y atar licencias; el código de solicitud.

use pos_licencia::{
    clave_publica_de, firmar, generar_clave_privada, leer, validar_equipo, ErrorLicencia, Huella,
    Licencia, Solicitud,
};

fn claves() -> ([u8; 32], [u8; 32]) {
    let privada = generar_clave_privada().unwrap();
    (privada, clave_publica_de(&privada))
}

fn equipo(n: &str) -> Huella {
    Huella::de("6F9619FF-8B86-D011-B42D-00C04FC964FF", n)
}

#[test]
fn emitir_y_verificar() {
    let (privada, publica) = claves();
    let lic = Licencia::nueva("Almacén Don Pepe", Some("76.123.456-7"), 1, None).unwrap();
    let texto = firmar(&lic, &privada).unwrap();
    assert!(texto.starts_with("CHUPS-"));
    // Bloques de 5 separados por guiones.
    assert!(texto["CHUPS-".len()..]
        .split('-')
        .all(|b| !b.is_empty() && b.len() <= 5));
    assert_eq!(leer(&texto, &publica).unwrap(), lic);
}

#[test]
fn atada_a_un_equipo() {
    let (privada, publica) = claves();
    let lic = Licencia::nueva("Kiosco", None, 1, Some(equipo("caja-1"))).unwrap();
    let leida = leer(&firmar(&lic, &privada).unwrap(), &publica).unwrap();
    assert_eq!(leida.equipo, Some(equipo("caja-1")));
    assert_eq!(leida.rut, None);
    assert_eq!(
        validar_equipo(&leida, &equipo("caja-1"), None),
        Ok(equipo("caja-1"))
    );
}

#[test]
fn firma_alterada_se_rechaza() {
    let (privada, publica) = claves();
    let lic = Licencia::nueva("Minimarket", None, 1, None).unwrap();
    let texto = firmar(&lic, &privada).unwrap();
    // Cambia un carácter de los datos (no del prefijo): el nombre o la firma ya no calzan.
    for posicion in [8, 20, texto.len() - 3] {
        let mut b: Vec<char> = texto.chars().collect();
        if b[posicion] == '-' {
            continue;
        }
        b[posicion] = if b[posicion] == 'A' { 'B' } else { 'A' };
        let alterado: String = b.into_iter().collect();
        assert!(
            matches!(
                leer(&alterado, &publica),
                Err(ErrorLicencia::Firma | ErrorLicencia::Formato(_))
            ),
            "posición {posicion}"
        );
    }
}

#[test]
fn otra_clave_publica_se_rechaza() {
    let (privada, _) = claves();
    let (_, otra) = claves();
    let texto = firmar(
        &Licencia::nueva("Botillería", None, 1, None).unwrap(),
        &privada,
    )
    .unwrap();
    assert_eq!(leer(&texto, &otra), Err(ErrorLicencia::Firma));
}

#[test]
fn licencia_de_otro_equipo_se_rechaza() {
    let lic = Licencia::nueva("Panadería", None, 1, Some(equipo("caja-1"))).unwrap();
    assert_eq!(
        validar_equipo(&lic, &equipo("caja-2"), None),
        Err(ErrorLicencia::OtroEquipo)
    );
}

#[test]
fn sin_equipo_se_ata_al_primero_y_no_a_otro() {
    let lic = Licencia::nueva("Verdulería", None, 1, None).unwrap();
    // Primera activación: se ata a este equipo.
    let atada = validar_equipo(&lic, &equipo("caja-1"), None).unwrap();
    assert_eq!(atada, equipo("caja-1"));
    // Después sigue valiendo ahí...
    assert_eq!(
        validar_equipo(&lic, &equipo("caja-1"), Some(&atada)),
        Ok(atada)
    );
    // ...y no en otro equipo con la misma base copiada.
    assert_eq!(
        validar_equipo(&lic, &equipo("caja-2"), Some(&atada)),
        Err(ErrorLicencia::OtroEquipo)
    );
}

#[test]
fn acepta_minusculas_guiones_espacios_y_comentarios() {
    let (privada, publica) = claves();
    let lic = Licencia::nueva("Ferretería Ñandú", Some("12.345.678-5"), 3, None).unwrap();
    let texto = firmar(&lic, &privada).unwrap();
    assert_eq!(leer(&texto.to_lowercase(), &publica).unwrap(), lic);
    let sin_guiones = texto.replace('-', "");
    assert_eq!(leer(&sin_guiones, &publica).unwrap(), lic);
    let partido = texto.replace('-', " - ").replace("CHUPS", "chups\n");
    assert_eq!(leer(&partido, &publica).unwrap(), lic);
    let archivo = format!(
        "# Licencia de CHUPS POS\n# Negocio: {}\n{texto}\n",
        lic.negocio
    );
    assert_eq!(leer(&archivo, &publica).unwrap(), lic);
}

#[test]
fn texto_incompleto_dice_que_falta() {
    let (privada, publica) = claves();
    let texto = firmar(&Licencia::nueva("Kiosco", None, 1, None).unwrap(), &privada).unwrap();
    let cortado = &texto[..texto.len() - 10];
    assert!(matches!(
        leer(cortado, &publica),
        Err(ErrorLicencia::Formato(_))
    ));
    assert!(matches!(leer("", &publica), Err(ErrorLicencia::Formato(_))));
    assert!(matches!(
        leer("hola", &publica),
        Err(ErrorLicencia::Formato(_))
    ));
}

#[test]
fn datos_invalidos_no_se_emiten() {
    assert!(Licencia::nueva("  ", None, 1, None).is_err());
    assert!(Licencia::nueva(&"x".repeat(101), None, 1, None).is_err());
    assert!(Licencia::nueva("Kiosco", None, 0, None).is_err());
}

#[test]
fn codigo_de_solicitud_ida_y_vuelta() {
    let s = Solicitud::nueva(equipo("caja-1"), "1.0.0");
    let texto = s.como_texto();
    assert_eq!(texto.replace('-', "").len(), 26);
    assert_eq!(Solicitud::desde_texto(&texto), Ok(s));
    assert_eq!(Solicitud::desde_texto(&texto.to_lowercase()), Ok(s));
    assert_eq!(s.version_texto(), "1.0.0");
    assert_eq!(
        Solicitud::nueva(equipo("x"), "1.2.3-rc.1").version,
        (1, 2, 3)
    );
}

#[test]
fn codigo_de_solicitud_con_un_error_de_tipeo_se_rechaza() {
    let texto = Solicitud::nueva(equipo("caja-1"), "1.0.0").como_texto();
    let mut b: Vec<char> = texto.chars().collect();
    b[2] = if b[2] == 'Q' { 'R' } else { 'Q' };
    let malo: String = b.into_iter().collect();
    assert_eq!(Solicitud::desde_texto(&malo), Err(ErrorLicencia::Solicitud));
}

#[test]
fn la_solicitud_da_una_licencia_para_ese_equipo() {
    let (privada, publica) = claves();
    let solicitud =
        Solicitud::desde_texto(&Solicitud::nueva(equipo("caja-7"), "1.0.0").como_texto()).unwrap();
    let lic = Licencia::nueva("Kiosco", None, 1, Some(solicitud.huella)).unwrap();
    let leida = leer(&firmar(&lic, &privada).unwrap(), &publica).unwrap();
    assert!(validar_equipo(&leida, &equipo("caja-7"), None).is_ok());
    assert!(validar_equipo(&leida, &equipo("caja-8"), None).is_err());
}

#[test]
fn la_clave_publica_incrustada_es_valida() {
    // Si clave-publica.txt no fuera una clave Ed25519 válida, ninguna licencia se podría activar.
    assert!(ed25519_dalek::VerifyingKey::from_bytes(&pos_licencia::CLAVE_PUBLICA).is_ok());
}
