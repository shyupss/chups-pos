// La impresora virtual "Archivo (pruebas)" funciona igual en Linux y en Windows.

use pos_hardware::{EstadoImpresora, Hardware, IMPRESORA_ARCHIVO};

#[test]
fn la_impresora_virtual_escribe_los_bytes_en_un_bin() {
    let carpeta = tempfile::tempdir().unwrap();
    let hw = Hardware::new(carpeta.path());

    let lista = hw.listar().unwrap();
    let virtual_ = lista.last().expect("al menos la virtual");
    assert_eq!(virtual_.nombre, IMPRESORA_ARCHIVO);
    assert!(virtual_.es_virtual);
    assert_eq!(
        hw.estado(IMPRESORA_ARCHIVO).unwrap(),
        EstadoImpresora::Lista
    );

    let bytes = [0x1b, b'@', b'H', b'o', b'l', b'a', 0x1d, b'V', 66, 0];
    hw.enviar(IMPRESORA_ARCHIVO, "boleta-000123", &bytes)
        .unwrap();
    let ruta = carpeta
        .path()
        .join("impresora-pruebas")
        .join("boleta-000123.bin");
    assert_eq!(std::fs::read(&ruta).unwrap(), bytes);

    // Un nombre de documento raro no sale de la carpeta.
    let ruta = hw.escribir_archivo("../fuera/ñ", b"x").unwrap();
    assert!(ruta.starts_with(hw.carpeta_pruebas()));
    assert_eq!(ruta.file_name().unwrap(), "___fuera__.bin");
}

#[test]
fn una_impresora_inexistente_da_error_y_no_bloquea() {
    let hw = Hardware::new(tempfile::tempdir().unwrap().path());
    assert!(hw.enviar("Impresora que no existe 123", "x", b"x").is_err());
}
