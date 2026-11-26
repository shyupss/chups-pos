//! Cola de impresión de Windows (winspool.drv).

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Graphics::Printing::{
    ClosePrinter, EndDocPrinter, EndPagePrinter, EnumPrintersW, GetDefaultPrinterW, GetPrinterW,
    OpenPrinterW, StartDocPrinterW, StartPagePrinter, WritePrinter, DOC_INFO_1W,
    PRINTER_ATTRIBUTE_WORK_OFFLINE, PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_HANDLE,
    PRINTER_INFO_2W,
};

use crate::{Error, EstadoImpresora, Impresora, Resultado};

// Bits de PRINTER_INFO_2.Status (winspool.h).
const PAUSADA: u32 = 0x1;
const ERROR: u32 = 0x2;
const ATASCO: u32 = 0x8;
const SIN_PAPEL: u32 = 0x10;
const PROBLEMA_PAPEL: u32 = 0x40;
const FUERA_DE_LINEA: u32 = 0x80;
const BANDEJA_LLENA: u32 = 0x800;
const NO_DISPONIBLE: u32 = 0x1000;
const SIN_TINTA: u32 = 0x40000;
const ATENCION: u32 = 0x100000;
const TAPA_ABIERTA: u32 = 0x400000;

fn ancho(texto: &str) -> Vec<u16> {
    texto.encode_utf16().chain(std::iter::once(0)).collect()
}

/// PWSTR terminado en 0 a String (vacío si es nulo).
fn texto(p: PWSTR) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { p.to_string() }.unwrap_or_default()
    }
}

fn error_sistema(que: &str) -> Error {
    Error::Sistema(format!("{que}: {}", windows::core::Error::from_thread()))
}

/// Traduce Status y Attributes de PRINTER_INFO_2 a algo que entienda el usuario.
fn estado_de(status: u32, atributos: u32) -> EstadoImpresora {
    if atributos & PRINTER_ATTRIBUTE_WORK_OFFLINE != 0
        || status & (FUERA_DE_LINEA | NO_DISPONIBLE) != 0
    {
        return EstadoImpresora::Desconectada;
    }
    let problemas = [
        (SIN_PAPEL, "Sin papel"),
        (ATASCO, "Papel atascado"),
        (PROBLEMA_PAPEL, "Problema con el papel"),
        (TAPA_ABIERTA, "Tapa abierta"),
        (SIN_TINTA, "Sin tinta"),
        (BANDEJA_LLENA, "Bandeja de salida llena"),
        (ATENCION, "Necesita atención"),
        (PAUSADA, "Pausada en Windows"),
        (ERROR, "Con error"),
    ];
    match problemas.iter().find(|(bit, _)| status & bit != 0) {
        Some((_, texto)) => EstadoImpresora::ConError((*texto).into()),
        None => EstadoImpresora::Lista,
    }
}

fn predeterminada() -> Option<String> {
    let mut largo = 0u32;
    unsafe {
        let _ = GetDefaultPrinterW(None, &mut largo);
    }
    if largo == 0 {
        return None;
    }
    let mut buffer = vec![0u16; largo as usize];
    let ok = unsafe { GetDefaultPrinterW(Some(PWSTR(buffer.as_mut_ptr())), &mut largo) };
    ok.as_bool().then(|| {
        String::from_utf16_lossy(&buffer[..buffer.iter().position(|&c| c == 0).unwrap_or(0)])
    })
}

/// Buffer alineado a 8 bytes para que Windows deje ahí las estructuras PRINTER_INFO_2W.
fn buffer_alineado(bytes: u32) -> Vec<u64> {
    vec![0u64; (bytes as usize).div_ceil(8)]
}

fn como_bytes(buffer: &mut [u64]) -> &mut [u8] {
    unsafe { std::slice::from_raw_parts_mut(buffer.as_mut_ptr().cast::<u8>(), buffer.len() * 8) }
}

pub fn listar() -> Resultado<Vec<Impresora>> {
    let banderas = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
    let (mut necesarios, mut cantidad) = (0u32, 0u32);
    // Primera llamada: solo pregunta cuánto espacio hace falta (falla a propósito).
    unsafe {
        let _ = EnumPrintersW(
            banderas,
            PCWSTR::null(),
            2,
            None,
            &mut necesarios,
            &mut cantidad,
        );
    }
    if necesarios == 0 {
        return Ok(Vec::new());
    }
    let mut buffer = buffer_alineado(necesarios);
    unsafe {
        EnumPrintersW(
            banderas,
            PCWSTR::null(),
            2,
            Some(como_bytes(&mut buffer)),
            &mut necesarios,
            &mut cantidad,
        )
    }
    .map_err(|e| Error::Sistema(format!("No se pudo ver las impresoras de Windows. Cierra y vuelve a abrir CHUPS POS; si sigue igual, avisa a quien te vendió el sistema. ({e})")))?;
    let infos = unsafe {
        std::slice::from_raw_parts(buffer.as_ptr().cast::<PRINTER_INFO_2W>(), cantidad as usize)
    };
    let defecto = predeterminada();
    let mut lista: Vec<Impresora> = infos
        .iter()
        .map(|i| {
            let nombre = texto(i.pPrinterName);
            Impresora {
                predeterminada: defecto.as_deref() == Some(nombre.as_str()),
                estado: estado_de(i.Status, i.Attributes),
                es_virtual: false,
                nombre,
            }
        })
        .filter(|i| !i.nombre.is_empty())
        .collect();
    lista.sort_by(|a, b| {
        b.predeterminada
            .cmp(&a.predeterminada)
            .then(a.nombre.cmp(&b.nombre))
    });
    Ok(lista)
}

/// Cierra la impresora al salir, pase lo que pase.
struct Manija(PRINTER_HANDLE);

impl Drop for Manija {
    fn drop(&mut self) {
        unsafe {
            let _ = ClosePrinter(self.0);
        }
    }
}

fn abrir(impresora: &str) -> Resultado<Manija> {
    let nombre = ancho(impresora);
    let mut manija = PRINTER_HANDLE::default();
    unsafe { OpenPrinterW(PCWSTR(nombre.as_ptr()), &mut manija, None) }
        .map_err(|_| Error::NoEncontrada(impresora.into()))?;
    Ok(Manija(manija))
}

pub fn estado(impresora: &str) -> Resultado<EstadoImpresora> {
    let manija = abrir(impresora)?;
    let mut necesarios = 0u32;
    unsafe {
        let _ = GetPrinterW(manija.0, 2, None, &mut necesarios);
    }
    if necesarios == 0 {
        return Ok(EstadoImpresora::Desconocido);
    }
    let mut buffer = buffer_alineado(necesarios);
    unsafe { GetPrinterW(manija.0, 2, Some(como_bytes(&mut buffer)), &mut necesarios) }
        .map_err(|e| Error::Sistema(format!("No se pudo saber si la impresora está lista. Revisa que esté encendida y conectada, y prueba de nuevo. ({e})")))?;
    let info = unsafe { &*buffer.as_ptr().cast::<PRINTER_INFO_2W>() };
    Ok(estado_de(info.Status, info.Attributes))
}

pub fn enviar(impresora: &str, documento: &str, bytes: &[u8]) -> Resultado<()> {
    let manija = abrir(impresora)?;
    let mut nombre_doc = ancho(documento);
    let mut tipo = ancho("RAW");
    let info = DOC_INFO_1W {
        pDocName: PWSTR(nombre_doc.as_mut_ptr()),
        pOutputFile: PWSTR::null(),
        pDatatype: PWSTR(tipo.as_mut_ptr()),
    };
    let trabajo = unsafe { StartDocPrinterW(manija.0, 1, &info) };
    if trabajo == 0 {
        return Err(error_sistema("La cola de impresión no aceptó el trabajo"));
    }
    let resultado = (|| {
        if !unsafe { StartPagePrinter(manija.0) }.as_bool() {
            return Err(error_sistema("No se pudo empezar a imprimir"));
        }
        let mut escritos = 0u32;
        let ok = unsafe {
            WritePrinter(
                manija.0,
                bytes.as_ptr().cast(),
                bytes.len() as u32,
                &mut escritos,
            )
        };
        unsafe {
            let _ = EndPagePrinter(manija.0);
        }
        if !ok.as_bool() || escritos as usize != bytes.len() {
            return Err(error_sistema("La impresora no recibió todo lo que se le mandó. Revisa el cable, apágala y préndela, y prueba de nuevo."));
        }
        Ok(())
    })();
    unsafe {
        let _ = EndDocPrinter(manija.0);
    }
    resultado
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traduce_el_estado_del_driver() {
        assert_eq!(estado_de(0, 0), EstadoImpresora::Lista);
        assert_eq!(estado_de(FUERA_DE_LINEA, 0), EstadoImpresora::Desconectada);
        assert_eq!(
            estado_de(0, PRINTER_ATTRIBUTE_WORK_OFFLINE),
            EstadoImpresora::Desconectada
        );
        assert_eq!(
            estado_de(SIN_PAPEL | ERROR, 0),
            EstadoImpresora::ConError("Sin papel".into())
        );
        assert_eq!(
            estado_de(PAUSADA, 0),
            EstadoImpresora::ConError("Pausada en Windows".into())
        );
    }

    #[test]
    fn lista_sin_fallar_y_no_encuentra_una_inexistente() {
        // En la CI de Windows puede no haber impresoras: solo no debe fallar.
        listar().unwrap();
        assert!(matches!(
            enviar("Impresora que no existe 123", "x", b"x"),
            Err(Error::NoEncontrada(_))
        ));
    }
}
