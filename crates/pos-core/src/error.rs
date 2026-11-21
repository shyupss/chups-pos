use thiserror::Error;

/// Errores del modelo. Se convierten a texto al cruzar hacia la interfaz.
#[derive(Debug, Error)]
pub enum Error {
    #[error("error de base de datos: {0}")]
    BaseDatos(#[from] sqlx::Error),

    #[error("no se pudo actualizar el esquema de la base de datos: {0}")]
    Migracion(#[from] sqlx::migrate::MigrateError),
}

pub type Resultado<T> = Result<T, Error>;
