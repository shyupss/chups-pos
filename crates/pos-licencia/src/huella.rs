//! Huella del equipo: identifica el computador donde se activó la licencia.
//!
//! Es un SHA-256 truncado a 10 bytes del identificador de la instalación de Windows (el
//! MachineGuid de `HKLM\SOFTWARE\Microsoft\Cryptography`) más el nombre del equipo. En Linux,
//! para desarrollo y la CI, usa `/etc/machine-id`. No depende de la red, de los discos ni de lo
//! que se conecte por USB, así que no cambia al reiniciar, al actualizar la app, al conectar un
//! pendrive o al cambiar de red. Cambia si se reinstala Windows o se renombra el equipo.

use sha2::{Digest, Sha256};

use crate::base32;

pub const LARGO_HUELLA: usize = 10;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Huella(pub [u8; LARGO_HUELLA]);

impl Huella {
    /// La huella de un identificador y un nombre de equipo. Ignora mayúsculas y espacios de los
    /// extremos, que Windows a veces cambia.
    pub fn de(identificador: &str, nombre_equipo: &str) -> Huella {
        let mut h = Sha256::new();
        h.update(b"chups-huella-v1|");
        h.update(identificador.trim().to_lowercase().as_bytes());
        h.update(b"|");
        h.update(nombre_equipo.trim().to_uppercase().as_bytes());
        let completo = h.finalize();
        let mut corta = [0u8; LARGO_HUELLA];
        corta.copy_from_slice(&completo[..LARGO_HUELLA]);
        Huella(corta)
    }

    /// 16 caracteres base32 en bloques: "ABCDE-FGHIJ-KLMNO-P".
    pub fn como_texto(&self) -> String {
        base32::en_bloques(&base32::codificar(&self.0))
    }

    pub fn desde_texto(texto: &str) -> Option<Huella> {
        let bytes = base32::decodificar(texto)?;
        let arreglo: [u8; LARGO_HUELLA] = bytes.try_into().ok()?;
        Some(Huella(arreglo))
    }
}

impl std::fmt::Debug for Huella {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Huella({})", self.como_texto())
    }
}

/// La huella de este computador.
pub fn huella_equipo() -> Result<Huella, String> {
    let (identificador, nombre) = datos_del_equipo()?;
    Ok(Huella::de(&identificador, &nombre))
}

#[cfg(windows)]
fn datos_del_equipo() -> Result<(String, String), String> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};
    use winreg::RegKey;

    let clave = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(
            r"SOFTWARE\Microsoft\Cryptography",
            KEY_READ | KEY_WOW64_64KEY,
        )
        .map_err(|e| format!("No se pudo leer el identificador de Windows: {e}"))?;
    let guid: String = clave
        .get_value("MachineGuid")
        .map_err(|e| format!("No se pudo leer el identificador de Windows: {e}"))?;
    let nombre = std::env::var("COMPUTERNAME").unwrap_or_default();
    Ok((guid, nombre))
}

#[cfg(not(windows))]
fn datos_del_equipo() -> Result<(String, String), String> {
    let identificador = ["/etc/machine-id", "/var/lib/dbus/machine-id"]
        .iter()
        .find_map(|r| std::fs::read_to_string(r).ok())
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| "No se encontró /etc/machine-id".to_owned())?;
    let nombre = std::fs::read_to_string("/etc/hostname")
        .ok()
        .or_else(|| std::env::var("HOSTNAME").ok())
        .unwrap_or_default();
    Ok((identificador, nombre))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn es_estable_y_tolera_mayusculas() {
        let a = Huella::de("6F9619FF-8B86-D011-B42D-00C04FC964FF", "caja-1");
        let b = Huella::de(" 6f9619ff-8b86-d011-b42d-00c04fc964ff ", "CAJA-1");
        assert_eq!(a, b);
        assert_ne!(
            a,
            Huella::de("6F9619FF-8B86-D011-B42D-00C04FC964FF", "caja-2")
        );
    }

    #[test]
    fn texto_ida_y_vuelta() {
        let h = Huella::de("x", "y");
        assert_eq!(Huella::desde_texto(&h.como_texto()), Some(h));
        assert_eq!(Huella::desde_texto(&h.como_texto().to_lowercase()), Some(h));
    }

    #[test]
    fn la_de_este_equipo_no_cambia_entre_lecturas() {
        if let Ok(h) = huella_equipo() {
            assert_eq!(huella_equipo().unwrap(), h);
        }
    }
}
