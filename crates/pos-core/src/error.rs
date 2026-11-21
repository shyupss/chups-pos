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
        }
        Error::BaseDatos(e)
    }
}

pub type Resultado<T> = Result<T, Error>;

pub(crate) fn validacion<T>(mensaje: impl Into<String>) -> Resultado<T> {
    Err(Error::Validacion(mensaje.into()))
}
