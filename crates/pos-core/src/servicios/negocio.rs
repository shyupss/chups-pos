//! Datos del negocio que salen en la boleta: nombre, RUT, dirección, teléfono, pie y logo.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::validacion;
use crate::repositorios::negocio::{self as repo, Campos, FilaNegocio};
use crate::servicios::seguridad::{self, Accion, ControlPin};
use crate::{logo, rut, Resultado};

/// Lo que ve la pantalla de Configuración.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Negocio {
    pub nombre: String,
    pub rut: Option<String>,
    pub direccion: Option<String>,
    pub telefono: Option<String>,
    pub mensaje_pie: String,
    /// PNG de 1 bit en base64, para mostrarlo como vista previa.
    pub logo_png: Option<String>,
}

/// Lo que llega desde el formulario (el logo va por separado).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatosNegocio {
    pub nombre: String,
    pub rut: Option<String>,
    pub direccion: Option<String>,
    pub telefono: Option<String>,
    pub mensaje_pie: String,
}

fn a_vista(f: FilaNegocio) -> Negocio {
    Negocio {
        nombre: f.nombre,
        rut: f.rut,
        direccion: f.direccion,
        telefono: f.telefono,
        mensaje_pie: f.mensaje_pie,
        logo_png: f.logo.as_deref().map(base64),
    }
}

/// Texto opcional: sin espacios sobrantes, vacío = no hay, y con largo máximo.
fn opcional(texto: Option<&str>, maximo: usize, que: &str) -> Resultado<Option<String>> {
    let texto = texto.map(str::trim).filter(|t| !t.is_empty());
    if texto.is_some_and(|t| t.chars().count() > maximo) {
        return validacion(format!("{que} no puede pasar de {maximo} caracteres"));
    }
    Ok(texto.map(str::to_owned))
}

pub async fn obtener(pool: &SqlitePool) -> Resultado<Negocio> {
    let mut conn = pool.acquire().await?;
    Ok(a_vista(repo::obtener(&mut conn).await?))
}

/// Guarda los datos. El nombre es obligatorio (sin él no se puede imprimir) y el RUT,
/// si viene, debe tener el dígito verificador correcto; se guarda como 12.345.678-5.
/// Pide el PIN si "cambiar configuración" está protegida.
pub async fn guardar(
    pool: &SqlitePool,
    control: &ControlPin,
    pin: Option<&str>,
    d: &DatosNegocio,
) -> Resultado<Negocio> {
    let nombre = d.nombre.trim();
    if nombre.is_empty() {
        return validacion("El nombre del negocio es obligatorio: sale arriba en cada boleta");
    }
    if nombre.chars().count() > 60 {
        return validacion("El nombre no puede pasar de 60 caracteres");
    }
    let rut = match d.rut.as_deref().map(str::trim).filter(|r| !r.is_empty()) {
        Some(r) => Some(rut::normalizar(r)?),
        None => None,
    };
    let direccion = opcional(d.direccion.as_deref(), 120, "La dirección")?;
    let telefono = opcional(d.telefono.as_deref(), 30, "El teléfono")?;
    let mensaje_pie = opcional(Some(&d.mensaje_pie), 200, "El mensaje al pie")?.unwrap_or_default();
    seguridad::exigir(pool, control, Accion::CambiarConfiguracion, pin).await?;
    repo::guardar(
        pool,
        &Campos {
            nombre,
            rut: rut.as_deref(),
            direccion: direccion.as_deref(),
            telefono: telefono.as_deref(),
            mensaje_pie: &mensaje_pie,
        },
    )
    .await?;
    obtener(pool).await
}

/// Procesa la imagen elegida (escala y blanco y negro) y la deja como logo.
pub async fn guardar_logo(
    pool: &SqlitePool,
    control: &ControlPin,
    pin: Option<&str>,
    archivo: &[u8],
) -> Resultado<Negocio> {
    let png = logo::procesar(archivo)?;
    seguridad::exigir(pool, control, Accion::CambiarConfiguracion, pin).await?;
    repo::guardar_logo(pool, Some(&png)).await?;
    obtener(pool).await
}

pub async fn quitar_logo(
    pool: &SqlitePool,
    control: &ControlPin,
    pin: Option<&str>,
) -> Resultado<Negocio> {
    seguridad::exigir(pool, control, Accion::CambiarConfiguracion, pin).await?;
    repo::guardar_logo(pool, None).await?;
    obtener(pool).await
}

/// Base64 estándar con relleno, suficiente para la vista previa del logo.
pub fn base64(bytes: &[u8]) -> String {
    const TABLA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut salida = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for trozo in bytes.chunks(3) {
        let n = (u32::from(trozo[0]) << 16)
            | (u32::from(*trozo.get(1).unwrap_or(&0)) << 8)
            | u32::from(*trozo.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= trozo.len() {
                salida.push(TABLA[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                salida.push('=');
            }
        }
    }
    salida
}

#[cfg(test)]
mod tests {
    use super::base64;

    #[test]
    fn codifica_base64() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(&[0xff, 0xfe]), "//4=");
    }
}
