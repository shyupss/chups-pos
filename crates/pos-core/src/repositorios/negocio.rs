use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::Resultado;

/// La fila única (id = 1) de la tabla negocio.
#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct FilaNegocio {
    pub nombre: String,
    pub rut: Option<String>,
    pub direccion: Option<String>,
    pub telefono: Option<String>,
    pub mensaje_pie: String,
    /// PNG de 1 bit.
    pub logo: Option<Vec<u8>>,
}

pub struct Campos<'a> {
    pub nombre: &'a str,
    pub rut: Option<&'a str>,
    pub direccion: Option<&'a str>,
    pub telefono: Option<&'a str>,
    pub mensaje_pie: &'a str,
}

pub async fn obtener(conn: &mut SqliteConnection) -> Resultado<FilaNegocio> {
    Ok(sqlx::query_as(
        "SELECT nombre, rut, direccion, telefono, mensaje_pie, logo FROM negocio WHERE id = 1",
    )
    .fetch_one(conn)
    .await?)
}

pub async fn guardar(pool: &SqlitePool, c: &Campos<'_>) -> Resultado<()> {
    sqlx::query(
        "UPDATE negocio SET nombre = ?, rut = ?, direccion = ?, telefono = ?, mensaje_pie = ?
         WHERE id = 1",
    )
    .bind(c.nombre)
    .bind(c.rut)
    .bind(c.direccion)
    .bind(c.telefono)
    .bind(c.mensaje_pie)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn guardar_logo(pool: &SqlitePool, logo: Option<&[u8]>) -> Resultado<()> {
    sqlx::query("UPDATE negocio SET logo = ? WHERE id = 1")
        .bind(logo)
        .execute(pool)
        .await?;
    Ok(())
}
