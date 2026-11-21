use thiserror::Error;

/// Errores del modelo. Se convierten a texto al cruzar hacia la interfaz,
/// así que los mensajes están escritos para el usuario del POS.
#[derive(Debug, Error)]
pub enum Error {
    /// Datos inválidos ingresados por el usuario (precio negativo, nombre vacío, etc.).
    #[error("{0}")]
    Validacion(String),

    #[error("{0}")]
    NoEncontrado(String),

    /// Choque con un dato que ya existe (por ejemplo, un código de barras repetido).
    #[error("{0}")]
    Conflicto(String),

    #[error("error de base de datos: {0}")]
    BaseDatos(sqlx::Error),

    #[error("no se pudo actualizar el esquema de la base de datos: {0}")]
    Migracion(#[from] sqlx::migrate::MigrateError),

    /// La acción está protegida y el PIN no vino, no coincide o está bloqueado.
    /// La interfaz lo distingue de los demás errores para volver a pedir el PIN.
    #[error("{0}")]
    Pin(FalloPin),
}

/// Por qué no se aceptó el PIN de administrador (o el código de recuperación).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FalloPin {
    Requerido,
    Incorrecto { quedan: u32 },
    CodigoIncorrecto { quedan: u32 },
    Bloqueado { segundos: u64 },
}

impl std::fmt::Display for FalloPin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let intentos = |n: u32| {
            if n == 1 {
                "te queda 1 intento".to_owned()
            } else {
                format!("te quedan {n} intentos")
            }
        };
        match self {
            FalloPin::Requerido => write!(f, "Esta acción pide el PIN de administrador"),
            FalloPin::Incorrecto { quedan } => write!(f, "PIN incorrecto: {}", intentos(*quedan)),
            FalloPin::CodigoIncorrecto { quedan } => {
                write!(
                    f,
                    "Código de recuperación incorrecto: {}",
                    intentos(*quedan)
                )
            }
            FalloPin::Bloqueado { segundos } => write!(
                f,
                "Demasiados intentos fallidos: espera {segundos} segundos para volver a probar"
            ),
        }
    }
}

impl From<sqlx::Error> for Error {
    fn from(e: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db) = &e {
            if db.is_unique_violation() && db.message().contains("codigo_barras") {
                return Error::Conflicto("Ya existe un producto con ese código de barras".into());
            }
            if db.is_unique_violation() && db.message().contains("categorias.nombre") {
                return Error::Conflicto("Ya existe una categoría con ese nombre".into());
            }
            if db.is_unique_violation() && db.message().contains("turnos.estado") {
                return Error::Conflicto("La caja ya está abierta".into());
            }
        }
        Error::BaseDatos(e)
    }
}

pub type Resultado<T> = Result<T, Error>;

pub(crate) fn validacion<T>(mensaje: impl Into<String>) -> Resultado<T> {
    Err(Error::Validacion(mensaje.into()))
}
