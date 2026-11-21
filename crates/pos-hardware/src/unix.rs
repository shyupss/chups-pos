//! Linux y macOS (desarrollo): CUPS por línea de comandos.
//! Sin CUPS instalado la lista queda vacía y solo funciona la impresora virtual.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::{Error, EstadoImpresora, Impresora, Resultado};

fn lpstat(argumentos: &[&str]) -> Option<String> {
    let salida = Command::new("lpstat").args(argumentos).output().ok()?;
    salida
        .status
        .success()
        .then(|| String::from_utf8_lossy(&salida.stdout).into_owned())
}

/// "printer POS-80 is idle.  enabled since…" / "printer X disabled since…"
fn estado_de(linea_lpstat: &str) -> EstadoImpresora {
    if linea_lpstat.contains("disabled") {
        EstadoImpresora::Desconectada
    } else if linea_lpstat.contains("idle") || linea_lpstat.contains("printing") {
        EstadoImpresora::Lista
    } else {
        EstadoImpresora::Desconocido
    }
}

pub fn listar() -> Resultado<Vec<Impresora>> {
    let Some(destinos) = lpstat(&["-e"]) else {
        return Ok(Vec::new());
    };
    let defecto = lpstat(&["-d"])
        .and_then(|t| t.rsplit(':').next().map(|n| n.trim().to_owned()))
        .unwrap_or_default();
    Ok(destinos
        .lines()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|nombre| Impresora {
            predeterminada: nombre == defecto,
            estado: estado(nombre).unwrap_or(EstadoImpresora::Desconocido),
            es_virtual: false,
            nombre: nombre.to_owned(),
        })
        .collect())
}

pub fn estado(impresora: &str) -> Resultado<EstadoImpresora> {
    match lpstat(&["-p", impresora]) {
        Some(linea) => Ok(estado_de(&linea)),
        None => Err(Error::NoEncontrada(impresora.into())),
    }
}

pub fn enviar(impresora: &str, documento: &str, bytes: &[u8]) -> Resultado<()> {
    let mut lp = Command::new("lp")
        .args(["-d", impresora, "-o", "raw", "-t", documento])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            Error::Sistema(format!(
                "No se pudo ejecutar lp (¿está instalado CUPS?): {e}"
            ))
        })?;
    lp.stdin
        .take()
        .expect("stdin con pipe")
        .write_all(bytes)
        .map_err(|e| Error::Sistema(format!("No se pudo mandar el trabajo a lp: {e}")))?;
    let salida = lp
        .wait_with_output()
        .map_err(|e| Error::Sistema(format!("lp no terminó: {e}")))?;
    if salida.status.success() {
        Ok(())
    } else {
        Err(Error::Sistema(format!(
            "lp rechazó el trabajo: {}",
            String::from_utf8_lossy(&salida.stderr).trim()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lee_el_estado_de_lpstat() {
        assert_eq!(
            estado_de("printer POS-80 is idle.  enabled since lun"),
            EstadoImpresora::Lista
        );
        assert_eq!(
            estado_de("printer POS-80 disabled since lun -"),
            EstadoImpresora::Desconectada
        );
        assert_eq!(estado_de("raro"), EstadoImpresora::Desconocido);
    }
}
