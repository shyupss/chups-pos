//! PIN de administrador para las acciones sensibles (anular, devolver, retirar efectivo, cerrar la
//! caja y cambiar la configuración).
//!
//! - El PIN (4 a 6 dígitos) y el código de recuperación se guardan solo como hash Argon2id.
//! - La verificación ocurre aquí: cada servicio protegido llama a [`exigir`] con el PIN que
//!   escribió el usuario. La interfaz nunca decide si el PIN es correcto.
//! - Tras [`MAX_INTENTOS`] fallos seguidos se bloquea por 30 segundos. El conteo vive en
//!   [`ControlPin`], en la memoria del proceso: reiniciar la app lo borra, pero no da más
//!   intentos que esperar.
//! - Sin PIN creado, nada se protege.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use argon2::password_hash::PasswordHasher;
use argon2::{Algorithm, Argon2, Params, PasswordVerifier, Version};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::validacion;
use crate::servicios::config::{self, AccionesProtegidas, Seguridad};
use crate::{Error, FalloPin, Resultado};

/// Fallos seguidos antes del bloqueo.
pub const MAX_INTENTOS: u32 = 5;
pub const BLOQUEO: Duration = Duration::from_secs(30);
/// Largo del código de recuperación.
pub const LARGO_CODIGO: usize = 10;
/// Base32 de Crockford: sin I, L, O ni U. Al leerlo, O vale 0 e I o L valen 1, así que
/// anotarlo mal no importa. 32 símbolos: 50 bits en total.
const ALFABETO_CODIGO: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Argon2id con 12 MiB y 2 pasadas: verificar tarda de 18 a 30 ms en un Core i5 de 11ª
/// generación (portátil), así que un PC cinco veces más lento sigue bajo los 150 ms. Un PIN de 4 a 6 dígitos no resiste
/// un ataque fuera de línea con ningún parámetro: lo que lo protege es el bloqueo tras 5 intentos.
/// Los parámetros quedan guardados en cada hash: cambiarlos no invalida los PIN existentes.
fn argon2() -> Argon2<'static> {
    let params = Params::new(12 * 1024, 2, 1, None).expect("parámetros de Argon2 válidos");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accion {
    AnularVenta,
    Devolucion,
    RetiroEfectivo,
    CerrarCaja,
    CambiarConfiguracion,
    AjustarInventario,
    FiarSobreLimite,
}

impl AccionesProtegidas {
    pub fn protege(&self, accion: Accion) -> bool {
        match accion {
            Accion::AnularVenta => self.anular_venta,
            Accion::Devolucion => self.devolucion,
            Accion::RetiroEfectivo => self.retiro_efectivo,
            Accion::CerrarCaja => self.cerrar_caja,
            Accion::CambiarConfiguracion => self.cambiar_configuracion,
            Accion::AjustarInventario => self.ajustar_inventario,
            Accion::FiarSobreLimite => self.fiar_sobre_limite,
        }
    }
}

/// Lo que muestra Configuración > Seguridad (sin hashes).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EstadoSeguridad {
    pub tiene_pin: bool,
    pub protegidas: AccionesProtegidas,
}

/// Fallos seguidos y bloqueo, en memoria del proceso. Uno por app (va en el estado de Tauri).
#[derive(Debug)]
pub struct ControlPin {
    intentos: Mutex<Intentos>,
    bloqueo: Duration,
}

#[derive(Debug, Default)]
struct Intentos {
    fallidos: u32,
    hasta: Option<Instant>,
}

impl Default for ControlPin {
    fn default() -> Self {
        Self::new()
    }
}

impl ControlPin {
    pub fn new() -> Self {
        Self::con_bloqueo(BLOQUEO)
    }

    /// Con otra duración de bloqueo (para las pruebas).
    pub fn con_bloqueo(bloqueo: Duration) -> Self {
        ControlPin {
            intentos: Mutex::new(Intentos::default()),
            bloqueo,
        }
    }

    fn intentos(&self) -> std::sync::MutexGuard<'_, Intentos> {
        // Un pánico con el candado tomado no deja datos a medias: se sigue usando.
        self.intentos.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Error si está bloqueado, con los segundos que faltan (redondeados hacia arriba).
    fn revisar_bloqueo(&self) -> Resultado<()> {
        let mut i = self.intentos();
        match i.hasta {
            Some(hasta) if hasta > Instant::now() => {
                let falta = hasta - Instant::now();
                let segundos = falta.as_millis().div_ceil(1000).max(1) as u64;
                Err(Error::Pin(FalloPin::Bloqueado { segundos }))
            }
            Some(_) => {
                i.hasta = None;
                Ok(())
            }
            None => Ok(()),
        }
    }

    /// Cuenta un fallo. Al quinto seguido, bloquea y vuelve a contar desde cero.
    fn fallo(&self, codigo: bool) -> Error {
        let mut i = self.intentos();
        i.fallidos += 1;
        if i.fallidos >= MAX_INTENTOS {
            i.fallidos = 0;
            i.hasta = Some(Instant::now() + self.bloqueo);
            return Error::Pin(FalloPin::Bloqueado {
                segundos: self.bloqueo.as_secs().max(1),
            });
        }
        let quedan = MAX_INTENTOS - i.fallidos;
        Error::Pin(if codigo {
            FalloPin::CodigoIncorrecto { quedan }
        } else {
            FalloPin::Incorrecto { quedan }
        })
    }

    fn exito(&self) {
        *self.intentos() = Intentos::default();
    }
}

/// Un PIN válido: solo dígitos, de 4 a 6.
fn formato_pin(pin: &str) -> bool {
    (4..=6).contains(&pin.len()) && pin.bytes().all(|b| b.is_ascii_digit())
}

fn validar_pin_nuevo(pin: &str) -> Resultado<()> {
    if !formato_pin(pin) {
        return validacion("El PIN debe tener de 4 a 6 dígitos");
    }
    Ok(())
}

/// El código tal como se anotó: sin espacios ni guiones, en mayúsculas y con O = 0, I = L = 1.
fn normalizar_codigo(codigo: &str) -> String {
    codigo
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        })
        .collect()
}

/// "ABCDE-FGHJK": así se muestra y se anota.
pub fn mostrar_codigo(codigo: &str) -> String {
    let mitad = codigo.len() / 2;
    format!("{}-{}", &codigo[..mitad], &codigo[mitad..])
}

fn codigo_nuevo() -> Resultado<String> {
    let mut bytes = [0u8; LARGO_CODIGO];
    getrandom::fill(&mut bytes).map_err(|e| {
        Error::Validacion(format!("No se pudo generar el código de recuperación: {e}"))
    })?;
    // 32 símbolos: los 5 bits bajos de cada byte eligen uno sin sesgo.
    Ok(bytes
        .iter()
        .map(|b| ALFABETO_CODIGO[usize::from(b & 31)] as char)
        .collect())
}

async fn hashear(secreto: String) -> Resultado<String> {
    tokio::task::spawn_blocking(move || {
        argon2()
            .hash_password(secreto.as_bytes())
            .map(|h| h.to_string())
            .map_err(|e| Error::Validacion(format!("No se pudo guardar el PIN: {e}")))
    })
    .await
    .map_err(|e| Error::Validacion(format!("No se pudo guardar el PIN: {e}")))?
}

/// Compara con el hash fuera de los hilos async. `false` también si el hash guardado está dañado.
async fn coincide(secreto: String, hash: String) -> bool {
    tokio::task::spawn_blocking(move || {
        argon2()
            .verify_password(secreto.as_bytes(), hash.as_str())
            .is_ok()
    })
    .await
    .unwrap_or(false)
}

/// Verifica `pin` contra el PIN guardado, contando fallos y respetando el bloqueo.
async fn verificar_pin(control: &ControlPin, hash: &str, pin: &str) -> Resultado<()> {
    control.revisar_bloqueo()?;
    // Lo que no tiene forma de PIN se rechaza sin gastar CPU, pero cuenta como fallo.
    if formato_pin(pin) && coincide(pin.to_owned(), hash.to_owned()).await {
        control.exito();
        Ok(())
    } else {
        Err(control.fallo(false))
    }
}

async fn verificar_codigo(control: &ControlPin, hash: &str, codigo: &str) -> Resultado<()> {
    control.revisar_bloqueo()?;
    let codigo = normalizar_codigo(codigo);
    if codigo.len() == LARGO_CODIGO && coincide(codigo, hash.to_owned()).await {
        control.exito();
        Ok(())
    } else {
        Err(control.fallo(true))
    }
}

pub async fn estado(pool: &SqlitePool) -> Resultado<EstadoSeguridad> {
    let s = config::seguridad(pool).await?;
    Ok(EstadoSeguridad {
        tiene_pin: s.pin_hash.is_some(),
        protegidas: s.protegidas,
    })
}

/// Deja pasar la acción o devuelve el error del PIN. Debe llamarse antes de abrir la transacción
/// de la acción (no deja conexiones tomadas mientras calcula el hash).
pub async fn exigir(
    pool: &SqlitePool,
    control: &ControlPin,
    accion: Accion,
    pin: Option<&str>,
) -> Resultado<()> {
    let s = config::seguridad(pool).await?;
    let Some(hash) = s.pin_hash else {
        return Ok(());
    };
    if !s.protegidas.protege(accion) {
        return Ok(());
    }
    match pin {
        None => Err(Error::Pin(FalloPin::Requerido)),
        Some(pin) => verificar_pin(control, &hash, pin).await,
    }
}

/// Crea el PIN cuando no hay ninguno. Devuelve el código de recuperación ("ABCDE-FGHJK"),
/// que se muestra una sola vez.
pub async fn crear_pin(pool: &SqlitePool, pin: &str) -> Resultado<String> {
    validar_pin_nuevo(pin)?;
    let mut s = config::seguridad(pool).await?;
    if s.pin_hash.is_some() {
        return Err(Error::Conflicto(
            "Ya hay un PIN: cámbialo con el PIN actual o usa el código de recuperación".into(),
        ));
    }
    let codigo = codigo_nuevo()?;
    s.pin_hash = Some(hashear(pin.to_owned()).await?);
    s.recuperacion_hash = Some(hashear(codigo.clone()).await?);
    config::guardar_seguridad(pool, &s).await?;
    Ok(mostrar_codigo(&codigo))
}

/// Lee la seguridad y exige que haya PIN.
async fn con_pin(pool: &SqlitePool) -> Resultado<(Seguridad, String)> {
    let s = config::seguridad(pool).await?;
    match s.pin_hash.clone() {
        Some(hash) => Ok((s, hash)),
        None => validacion("No hay un PIN creado"),
    }
}

/// Cambia el PIN con el actual. El código de recuperación sigue siendo el mismo.
pub async fn cambiar_pin(
    pool: &SqlitePool,
    control: &ControlPin,
    actual: &str,
    nuevo: &str,
) -> Resultado<()> {
    validar_pin_nuevo(nuevo)?;
    let (mut s, hash) = con_pin(pool).await?;
    verificar_pin(control, &hash, actual).await?;
    s.pin_hash = Some(hashear(nuevo.to_owned()).await?);
    config::guardar_seguridad(pool, &s).await
}

/// Quita el PIN (y su código): desde ahí nada se protege.
pub async fn quitar_pin(pool: &SqlitePool, control: &ControlPin, actual: &str) -> Resultado<()> {
    let (mut s, hash) = con_pin(pool).await?;
    verificar_pin(control, &hash, actual).await?;
    s.pin_hash = None;
    s.recuperacion_hash = None;
    config::guardar_seguridad(pool, &s).await
}

/// Pone un PIN nuevo con el código de recuperación. El código usado deja de servir: se devuelve
/// uno nuevo para anotar.
pub async fn recuperar_pin(
    pool: &SqlitePool,
    control: &ControlPin,
    codigo: &str,
    nuevo: &str,
) -> Resultado<String> {
    validar_pin_nuevo(nuevo)?;
    let (mut s, _) = con_pin(pool).await?;
    let Some(hash_codigo) = s.recuperacion_hash.clone() else {
        return validacion("Este PIN no tiene código de recuperación: quítalo con el PIN actual");
    };
    verificar_codigo(control, &hash_codigo, codigo).await?;
    let codigo = codigo_nuevo()?;
    s.pin_hash = Some(hashear(nuevo.to_owned()).await?);
    s.recuperacion_hash = Some(hashear(codigo.clone()).await?);
    config::guardar_seguridad(pool, &s).await?;
    Ok(mostrar_codigo(&codigo))
}

/// Cambia qué acciones piden el PIN. Si hay PIN, siempre lo pide (aunque "cambiar
/// configuración" no esté protegida): si no, cualquiera podría desprotegerlo todo.
pub async fn guardar_protegidas(
    pool: &SqlitePool,
    control: &ControlPin,
    protegidas: AccionesProtegidas,
    pin: Option<&str>,
) -> Resultado<EstadoSeguridad> {
    let mut s = config::seguridad(pool).await?;
    if let Some(hash) = &s.pin_hash {
        match pin {
            None => return Err(Error::Pin(FalloPin::Requerido)),
            Some(pin) => verificar_pin(control, hash, pin).await?,
        }
    }
    s.protegidas = protegidas;
    config::guardar_seguridad(pool, &s).await?;
    estado(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formato_del_pin() {
        assert!(formato_pin("1234"));
        assert!(formato_pin("123456"));
        assert!(!formato_pin("123"));
        assert!(!formato_pin("1234567"));
        assert!(!formato_pin("12a4"));
        assert!(!formato_pin("١٢٣٤")); // dígitos que no son ASCII
    }

    /// Para revisar el tiempo en un PC modesto: `cargo test --release -p pos-core -- --ignored mide`.
    #[test]
    #[ignore]
    fn mide_la_verificacion() {
        let hash = argon2().hash_password(b"4821").unwrap().to_string();
        let inicio = std::time::Instant::now();
        for _ in 0..10 {
            assert!(argon2().verify_password(b"4821", hash.as_str()).is_ok());
        }
        println!("verificar el PIN: {:?} cada vez", inicio.elapsed() / 10);
    }

    #[test]
    fn codigo_de_recuperacion() {
        let c = codigo_nuevo().unwrap();
        assert_eq!(c.len(), LARGO_CODIGO);
        assert!(c.bytes().all(|b| ALFABETO_CODIGO.contains(&b)));
        assert_eq!(mostrar_codigo("ABCDEFGHJK"), "ABCDE-FGHJK");
        assert_eq!(normalizar_codigo(" abcde-fghjk "), "ABCDEFGHJK");
        assert_eq!(normalizar_codigo("o1il0-AB"), "01110AB");
    }
}
