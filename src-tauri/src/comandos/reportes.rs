//! Reportes: la pantalla Hoy y los reportes por rango, en el día y la hora local del computador.

use chrono::Utc;
use pos_core::servicios::reportes::{
    self as srv, Rango, ReporteFiados, ReporteGanancia, ReporteProductos, ReporteTurnos,
    ReporteVentas, ResumenHoy,
};
use pos_core::zona::ZonaLocal;
use tauri::State;

use super::{a_texto, ResultadoComando};
use crate::estado::EstadoApp;

fn rango(desde: &str, hasta: &str) -> ResultadoComando<Rango> {
    Rango::de_textos(desde, hasta).map_err(a_texto)
}

/// Todo lo de la pantalla Hoy en una llamada.
#[tauri::command]
pub async fn reporte_hoy(estado: State<'_, EstadoApp>) -> ResultadoComando<ResumenHoy> {
    srv::hoy(&estado.pool, &ZonaLocal, Utc::now())
        .await
        .map_err(a_texto)
}

#[tauri::command]
pub async fn reporte_ventas(
    estado: State<'_, EstadoApp>,
    desde: String,
    hasta: String,
) -> ResultadoComando<ReporteVentas> {
    srv::ventas(&estado.pool, &ZonaLocal, rango(&desde, &hasta)?)
        .await
        .map_err(a_texto)
}

/// `categoria`: null todas, 0 sin categoría.
#[tauri::command]
pub async fn reporte_productos(
    estado: State<'_, EstadoApp>,
    desde: String,
    hasta: String,
    categoria: Option<i64>,
) -> ResultadoComando<ReporteProductos> {
    srv::productos_vendidos(&estado.pool, &ZonaLocal, rango(&desde, &hasta)?, categoria)
        .await
        .map_err(a_texto)
}

#[tauri::command]
pub async fn reporte_ganancia(
    estado: State<'_, EstadoApp>,
    desde: String,
    hasta: String,
) -> ResultadoComando<ReporteGanancia> {
    srv::ganancia(&estado.pool, &ZonaLocal, rango(&desde, &hasta)?)
        .await
        .map_err(a_texto)
}

#[tauri::command]
pub async fn reporte_turnos(
    estado: State<'_, EstadoApp>,
    desde: String,
    hasta: String,
) -> ResultadoComando<ReporteTurnos> {
    srv::turnos(&estado.pool, &ZonaLocal, rango(&desde, &hasta)?)
        .await
        .map_err(a_texto)
}

#[tauri::command]
pub async fn reporte_fiados(
    estado: State<'_, EstadoApp>,
    desde: String,
    hasta: String,
) -> ResultadoComando<ReporteFiados> {
    srv::fiados(&estado.pool, &ZonaLocal, rango(&desde, &hasta)?, Utc::now())
        .await
        .map_err(a_texto)
}
