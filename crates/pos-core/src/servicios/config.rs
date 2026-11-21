//! Configuración tipada sobre la tabla `config` (clave/valor de texto).
//!
//! Es el único lugar que conoce las claves: el resto del código usa [`Config`].
//! Una clave que falta o trae un valor raro vale lo mismo que el valor por defecto.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

use crate::error::validacion;
use crate::repositorios::config as repo;
use crate::servicios::seguridad::{self, Accion, ControlPin};
use crate::Resultado;

/// Tabla de caracteres de la impresora térmica. Cada marca responde distinto, por eso se elige.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TablaCaracteres {
    /// PC858: la 850 con el signo €. Es la que mejor funciona en la mayoría.
    #[default]
    Cp858,
    Cp850,
    Wpc1252,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormatoPdf {
    /// 80 mm de ancho y el alto que necesite el contenido.
    #[default]
    Ticket,
    Carta,
}

/// Qué traen las etiquetas de la balanza después del código de producto.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormatoBalanza {
    #[default]
    Peso,
    Precio,
}

/// Nombre con que cada enum se guarda en la tabla config (el mismo que usa la vista).
macro_rules! enum_de_texto {
    ($tipo:ty { $($variante:ident => $texto:literal),+ $(,)? }) => {
        impl $tipo {
            pub fn como_texto(self) -> &'static str {
                match self { $(Self::$variante => $texto),+ }
            }
            pub fn desde_texto(texto: &str) -> Option<Self> {
                match texto { $($texto => Some(Self::$variante),)+ _ => None }
            }
        }
    };
}

enum_de_texto!(TablaCaracteres { Cp858 => "cp858", Cp850 => "cp850", Wpc1252 => "wpc1252" });
enum_de_texto!(FormatoPdf { Ticket => "ticket", Carta => "carta" });
enum_de_texto!(FormatoBalanza { Peso => "peso", Precio => "precio" });

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    // Caja
    /// Redondear el efectivo a la decena.
    pub redondeo_efectivo: bool,

    // Boleta
    /// "Comprobante interno sin validez tributaria" al pie.
    pub boleta_leyenda: bool,
    /// Código de barras CODE128 con el folio al pie.
    pub boleta_codigo: bool,

    // Impresora
    /// Nombre de la impresora de Windows (o "Archivo (pruebas)"). Sin elegir, no se imprime.
    pub impresora: Option<String>,
    /// 58 u 80 mm.
    pub ancho_papel_mm: u8,
    pub tabla_caracteres: TablaCaracteres,
    pub imprimir_al_cobrar: bool,
    /// 1 o 2.
    pub copias: u8,
    pub pdf_formato: FormatoPdf,
    /// Carpeta de los PDF; sin valor, Documentos\CHUPS POS\Boletas.
    pub pdf_carpeta: Option<String>,

    // Cajón (conectado a la impresora)
    pub cajon_activo: bool,
    /// Pin del conector: 2 o 5.
    pub cajon_pin: u8,
    /// Duración del pulso en milisegundos.
    pub cajon_pulso_ms: u16,
    pub cajon_solo_efectivo: bool,

    // Códigos de balanza
    pub balanza_activa: bool,
    /// Prefijos de dos dígitos, como "20-29" o "20,21,25".
    pub balanza_prefijos: String,
    pub balanza_formato: FormatoBalanza,
    /// Dígitos del código de producto dentro de la etiqueta (4 a 6).
    pub balanza_largo_codigo: u8,

    // Fiados
    /// Días desde la venta fiada más antigua a partir de los que la deuda se resalta.
    pub fiado_dias_alerta: u16,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            redondeo_efectivo: true,
            boleta_leyenda: true,
            boleta_codigo: true,
            impresora: None,
            ancho_papel_mm: 80,
            tabla_caracteres: TablaCaracteres::Cp858,
            imprimir_al_cobrar: true,
            copias: 1,
            pdf_formato: FormatoPdf::Ticket,
            pdf_carpeta: None,
            cajon_activo: false,
            cajon_pin: 2,
            cajon_pulso_ms: 100,
            cajon_solo_efectivo: true,
            balanza_activa: false,
            balanza_prefijos: "20-29".into(),
            balanza_formato: FormatoBalanza::Peso,
            balanza_largo_codigo: 5,
            fiado_dias_alerta: 30,
        }
    }
}

impl Config {
    /// Caracteres por línea según el ancho del papel.
    pub fn columnas(&self) -> usize {
        if self.ancho_papel_mm == 58 {
            32
        } else {
            48
        }
    }

    /// Prefijos de balanza ya expandidos: "20-22,25" -> [20, 21, 22, 25].
    pub fn prefijos_balanza(&self) -> Vec<u8> {
        leer_prefijos(&self.balanza_prefijos).unwrap_or_default()
    }
}

/// Acciones que pueden pedir el PIN de administrador. Sin PIN creado, ninguna lo pide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccionesProtegidas {
    pub anular_venta: bool,
    pub devolucion: bool,
    pub retiro_efectivo: bool,
    pub cerrar_caja: bool,
    pub cambiar_configuracion: bool,
    /// Ajustes de stock por conteo, mermas y anular compras.
    pub ajustar_inventario: bool,
    /// Vender fiado por sobre el límite de crédito del cliente.
    pub fiar_sobre_limite: bool,
}

impl Default for AccionesProtegidas {
    fn default() -> Self {
        AccionesProtegidas {
            anular_venta: true,
            devolucion: true,
            retiro_efectivo: true,
            cerrar_caja: false,
            cambiar_configuracion: true,
            ajustar_inventario: true,
            fiar_sobre_limite: true,
        }
    }
}

/// PIN y acciones protegidas. Va aparte de [`Config`] porque los hashes nunca salen hacia la
/// interfaz: `obtener_config` no los incluye y `guardar` no los toca.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Seguridad {
    /// Hash Argon2id del PIN (formato PHC, con su sal); `None` si no hay PIN.
    pub pin_hash: Option<String>,
    /// Hash del código de recuperación que se mostró una vez al crear el PIN.
    pub recuperacion_hash: Option<String>,
    pub protegidas: AccionesProtegidas,
}

/// "20-29" o "20, 21, 25" -> lista de prefijos de dos dígitos. `None` si el texto no sirve.
fn leer_prefijos(texto: &str) -> Option<Vec<u8>> {
    let mut prefijos = Vec::new();
    for parte in texto.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let dos_digitos = |t: &str| (t.len() == 2).then(|| t.parse::<u8>().ok()).flatten();
        match parte.split_once('-') {
            Some((desde, hasta)) => {
                let (desde, hasta) = (dos_digitos(desde.trim())?, dos_digitos(hasta.trim())?);
                if desde > hasta {
                    return None;
                }
                prefijos.extend(desde..=hasta);
            }
            None => prefijos.push(dos_digitos(parte)?),
        }
    }
    prefijos.sort_unstable();
    prefijos.dedup();
    (!prefijos.is_empty()).then_some(prefijos)
}

// Claves de la tabla config. Ningún otro archivo las escribe.
const REDONDEO_EFECTIVO: &str = "redondeo_efectivo";
const BOLETA_LEYENDA: &str = "boleta_leyenda";
const BOLETA_CODIGO: &str = "boleta_codigo";
const IMPRESORA: &str = "impresora";
const ANCHO_PAPEL: &str = "ancho_papel_mm";
const TABLA_CARACTERES: &str = "tabla_caracteres";
const IMPRIMIR_AL_COBRAR: &str = "imprimir_al_cobrar";
const COPIAS: &str = "copias";
const PDF_FORMATO: &str = "pdf_formato";
const PDF_CARPETA: &str = "pdf_carpeta";
const CAJON_ACTIVO: &str = "cajon_activo";
const CAJON_PIN: &str = "cajon_pin";
const CAJON_PULSO_MS: &str = "cajon_pulso_ms";
const CAJON_SOLO_EFECTIVO: &str = "cajon_solo_efectivo";
const BALANZA_ACTIVA: &str = "balanza_activa";
const BALANZA_PREFIJOS: &str = "balanza_prefijos";
const BALANZA_FORMATO: &str = "balanza_formato";
const BALANZA_LARGO_CODIGO: &str = "balanza_largo_codigo";
const FIADO_DIAS_ALERTA: &str = "fiado_dias_alerta";
// Seguridad: fuera de `Config`.
const PIN_HASH: &str = "pin_hash";
const RECUPERACION_HASH: &str = "pin_recuperacion_hash";
const PROTEGER_ANULAR: &str = "proteger_anular_venta";
const PROTEGER_DEVOLUCION: &str = "proteger_devolucion";
const PROTEGER_RETIRO: &str = "proteger_retiro_efectivo";
const PROTEGER_CIERRE: &str = "proteger_cerrar_caja";
const PROTEGER_CONFIG: &str = "proteger_cambiar_configuracion";
const PROTEGER_INVENTARIO: &str = "proteger_ajustar_inventario";
const PROTEGER_LIMITE: &str = "proteger_fiar_sobre_limite";

fn si_no(valor: bool) -> String {
    if valor { "1" } else { "0" }.into()
}

fn bool_de(claves: &HashMap<String, String>, clave: &str, defecto: bool) -> bool {
    match claves.get(clave).map(String::as_str) {
        Some("1") => true,
        Some("0") => false,
        _ => defecto,
    }
}

impl Seguridad {
    fn desde_claves(claves: &HashMap<String, String>) -> Seguridad {
        let d = AccionesProtegidas::default();
        let hash = |k: &str| claves.get(k).filter(|v| !v.is_empty()).cloned();
        Seguridad {
            pin_hash: hash(PIN_HASH),
            recuperacion_hash: hash(RECUPERACION_HASH),
            protegidas: AccionesProtegidas {
                anular_venta: bool_de(claves, PROTEGER_ANULAR, d.anular_venta),
                devolucion: bool_de(claves, PROTEGER_DEVOLUCION, d.devolucion),
                retiro_efectivo: bool_de(claves, PROTEGER_RETIRO, d.retiro_efectivo),
                cerrar_caja: bool_de(claves, PROTEGER_CIERRE, d.cerrar_caja),
                cambiar_configuracion: bool_de(claves, PROTEGER_CONFIG, d.cambiar_configuracion),
                ajustar_inventario: bool_de(claves, PROTEGER_INVENTARIO, d.ajustar_inventario),
                fiar_sobre_limite: bool_de(claves, PROTEGER_LIMITE, d.fiar_sobre_limite),
            },
        }
    }

    fn a_claves(&self) -> [(&'static str, String); 9] {
        let p = &self.protegidas;
        [
            (PIN_HASH, self.pin_hash.clone().unwrap_or_default()),
            (
                RECUPERACION_HASH,
                self.recuperacion_hash.clone().unwrap_or_default(),
            ),
            (PROTEGER_ANULAR, si_no(p.anular_venta)),
            (PROTEGER_DEVOLUCION, si_no(p.devolucion)),
            (PROTEGER_RETIRO, si_no(p.retiro_efectivo)),
            (PROTEGER_CIERRE, si_no(p.cerrar_caja)),
            (PROTEGER_CONFIG, si_no(p.cambiar_configuracion)),
            (PROTEGER_INVENTARIO, si_no(p.ajustar_inventario)),
            (PROTEGER_LIMITE, si_no(p.fiar_sobre_limite)),
        ]
    }
}

impl Config {
    fn desde_claves(claves: &HashMap<String, String>) -> Config {
        let d = Config::default();
        let texto = |k: &str| claves.get(k).map(String::as_str);
        let bool_de = |k: &str, defecto: bool| bool_de(claves, k, defecto);
        Config {
            redondeo_efectivo: bool_de(REDONDEO_EFECTIVO, d.redondeo_efectivo),
            boleta_leyenda: bool_de(BOLETA_LEYENDA, d.boleta_leyenda),
            boleta_codigo: bool_de(BOLETA_CODIGO, d.boleta_codigo),
            impresora: texto(IMPRESORA)
                .filter(|v| !v.is_empty())
                .map(str::to_owned),
            ancho_papel_mm: match texto(ANCHO_PAPEL) {
                Some("58") => 58,
                _ => d.ancho_papel_mm,
            },
            tabla_caracteres: texto(TABLA_CARACTERES)
                .and_then(TablaCaracteres::desde_texto)
                .unwrap_or(d.tabla_caracteres),
            imprimir_al_cobrar: bool_de(IMPRIMIR_AL_COBRAR, d.imprimir_al_cobrar),
            copias: match texto(COPIAS) {
                Some("2") => 2,
                _ => d.copias,
            },
            pdf_formato: texto(PDF_FORMATO)
                .and_then(FormatoPdf::desde_texto)
                .unwrap_or(d.pdf_formato),
            pdf_carpeta: texto(PDF_CARPETA)
                .filter(|v| !v.is_empty())
                .map(str::to_owned),
            cajon_activo: bool_de(CAJON_ACTIVO, d.cajon_activo),
            cajon_pin: match texto(CAJON_PIN) {
                Some("5") => 5,
                _ => d.cajon_pin,
            },
            cajon_pulso_ms: texto(CAJON_PULSO_MS)
                .and_then(|v| v.parse().ok())
                .filter(|ms| (10..=500).contains(ms))
                .unwrap_or(d.cajon_pulso_ms),
            cajon_solo_efectivo: bool_de(CAJON_SOLO_EFECTIVO, d.cajon_solo_efectivo),
            balanza_activa: bool_de(BALANZA_ACTIVA, d.balanza_activa),
            balanza_prefijos: texto(BALANZA_PREFIJOS)
                .filter(|v| leer_prefijos(v).is_some())
                .map(str::to_owned)
                .unwrap_or(d.balanza_prefijos),
            balanza_formato: texto(BALANZA_FORMATO)
                .and_then(FormatoBalanza::desde_texto)
                .unwrap_or(d.balanza_formato),
            balanza_largo_codigo: texto(BALANZA_LARGO_CODIGO)
                .and_then(|v| v.parse().ok())
                .filter(|n| (4..=6).contains(n))
                .unwrap_or(d.balanza_largo_codigo),
            fiado_dias_alerta: texto(FIADO_DIAS_ALERTA)
                .and_then(|v| v.parse().ok())
                .filter(|n| (1..=365).contains(n))
                .unwrap_or(d.fiado_dias_alerta),
        }
    }

    fn a_claves(&self) -> Vec<(&'static str, String)> {
        vec![
            (REDONDEO_EFECTIVO, si_no(self.redondeo_efectivo)),
            (BOLETA_LEYENDA, si_no(self.boleta_leyenda)),
            (BOLETA_CODIGO, si_no(self.boleta_codigo)),
            (IMPRESORA, self.impresora.clone().unwrap_or_default()),
            (ANCHO_PAPEL, self.ancho_papel_mm.to_string()),
            (TABLA_CARACTERES, self.tabla_caracteres.como_texto().into()),
            (IMPRIMIR_AL_COBRAR, si_no(self.imprimir_al_cobrar)),
            (COPIAS, self.copias.to_string()),
            (PDF_FORMATO, self.pdf_formato.como_texto().into()),
            (PDF_CARPETA, self.pdf_carpeta.clone().unwrap_or_default()),
            (CAJON_ACTIVO, si_no(self.cajon_activo)),
            (CAJON_PIN, self.cajon_pin.to_string()),
            (CAJON_PULSO_MS, self.cajon_pulso_ms.to_string()),
            (CAJON_SOLO_EFECTIVO, si_no(self.cajon_solo_efectivo)),
            (BALANZA_ACTIVA, si_no(self.balanza_activa)),
            (BALANZA_PREFIJOS, self.balanza_prefijos.clone()),
            (BALANZA_FORMATO, self.balanza_formato.como_texto().into()),
            (BALANZA_LARGO_CODIGO, self.balanza_largo_codigo.to_string()),
            (FIADO_DIAS_ALERTA, self.fiado_dias_alerta.to_string()),
        ]
    }

    /// Revisa y limpia lo que llega desde la pantalla antes de guardarlo.
    fn validar(mut self) -> Resultado<Config> {
        if ![58, 80].contains(&self.ancho_papel_mm) {
            return validacion("El ancho del papel debe ser 58 u 80 mm");
        }
        if ![1, 2].contains(&self.copias) {
            return validacion("Las copias deben ser 1 o 2");
        }
        if ![2, 5].contains(&self.cajon_pin) {
            return validacion("El cajón se conecta al pin 2 o al pin 5");
        }
        if !(10..=500).contains(&self.cajon_pulso_ms) {
            return validacion("El pulso del cajón debe durar entre 10 y 500 ms");
        }
        if !(1..=365).contains(&self.fiado_dias_alerta) {
            return validacion("La alerta de fiados debe estar entre 1 y 365 días");
        }
        if !(4..=6).contains(&self.balanza_largo_codigo) {
            return validacion(
                "El código de producto de la balanza debe tener entre 4 y 6 dígitos",
            );
        }
        let Some(prefijos) = leer_prefijos(&self.balanza_prefijos) else {
            return validacion("Los prefijos de balanza van de dos dígitos, como 20-29 o 20,21,25");
        };
        if prefijos.iter().any(|p| !(20..=29).contains(p)) {
            return validacion(
                "Los prefijos de balanza deben estar entre 20 y 29 (códigos de uso interno)",
            );
        }
        self.balanza_prefijos = self
            .balanza_prefijos
            .split(',')
            .map(|p| p.split_whitespace().collect::<String>())
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(",");
        self.impresora = self
            .impresora
            .map(|i| i.trim().to_owned())
            .filter(|i| !i.is_empty());
        self.pdf_carpeta = self
            .pdf_carpeta
            .map(|c| c.trim().to_owned())
            .filter(|c| !c.is_empty());
        Ok(self)
    }
}

pub async fn obtener_con(conn: &mut SqliteConnection) -> Resultado<Config> {
    Ok(Config::desde_claves(&repo::leer_todo(conn).await?))
}

pub async fn obtener(pool: &SqlitePool) -> Resultado<Config> {
    let mut conn = pool.acquire().await?;
    obtener_con(&mut conn).await
}

/// Guarda toda la configuración en una transacción y devuelve lo que quedó.
/// Pide el PIN si "cambiar configuración" está protegida.
pub async fn guardar(
    pool: &SqlitePool,
    control: &ControlPin,
    pin: Option<&str>,
    nueva: &Config,
) -> Resultado<Config> {
    let limpia = nueva.clone().validar()?;
    seguridad::exigir(pool, control, Accion::CambiarConfiguracion, pin).await?;
    let mut tx = pool.begin().await?;
    for (clave, valor) in limpia.a_claves() {
        repo::guardar(&mut tx, clave, &valor).await?;
    }
    tx.commit().await?;
    obtener(pool).await
}

pub async fn seguridad(pool: &SqlitePool) -> Resultado<Seguridad> {
    let mut conn = pool.acquire().await?;
    Ok(Seguridad::desde_claves(&repo::leer_todo(&mut conn).await?))
}

/// Guarda PIN, código de recuperación y acciones protegidas. Quien llama ya validó el PIN.
pub(crate) async fn guardar_seguridad(pool: &SqlitePool, s: &Seguridad) -> Resultado<()> {
    let mut tx = pool.begin().await?;
    for (clave, valor) in s.a_claves() {
        repo::guardar(&mut tx, clave, &valor).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Lo que necesita el servicio de ventas al cobrar, dentro de su transacción.
pub async fn redondeo_efectivo_activo(conn: &mut SqliteConnection) -> Resultado<bool> {
    Ok(obtener_con(conn).await?.redondeo_efectivo)
}
