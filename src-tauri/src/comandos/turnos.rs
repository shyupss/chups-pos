//! Turnos de caja: abrir, ingresos y retiros, resumen y cierre (con su impresión y PDF).

use pos_core::servicios::impresion;
use pos_core::servicios::turnos::{
    self as srv, Conteo, EstadoCaja, MovimientoCaja, ResumenTurno, TipoMovimiento, Turno,
    TurnoBreve,
};
use tauri::{AppHandle, State};

use super::{a_texto, ResultadoComando, ResultadoProtegido};
use crate::estado::EstadoApp;

/// El turno abierto (o, si no hay, el último monto inicial para sugerirlo).
#[tauri::command]
pub async fn estado_caja(estado: State<'_, EstadoApp>) -> ResultadoComando<EstadoCaja> {
    srv::estado_caja(&estado.pool).await.map_err(a_texto)
}

#[tauri::command]
pub async fn abrir_turno(
    estado: State<'_, EstadoApp>,
    monto_inicial: i64,
) -> ResultadoComando<Turno> {
    srv::abrir(&estado.pool, monto_inicial)
        .await
        .map_err(a_texto)
}

/// Ingreso o retiro de efectivo. El retiro pide el PIN si está protegido.
#[tauri::command]
pub async fn registrar_movimiento_caja(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
    tipo: TipoMovimiento,
    monto: i64,
    motivo: String,
    pin: Option<String>,
) -> ResultadoProtegido<MovimientoCaja> {
    let r = srv::registrar_movimiento(
        &estado.pool,
        &estado.pin,
        tipo,
        monto,
        &motivo,
        pin.as_deref(),
    )
    .await?;
    super::avisar_cambios(&app);
    Ok(r)
}

#[tauri::command]
pub async fn resumen_turno(
    estado: State<'_, EstadoApp>,
    turno_id: i64,
) -> ResultadoComando<ResumenTurno> {
    srv::resumen(&estado.pool, turno_id).await.map_err(a_texto)
}

/// Cierra la caja con el efectivo contado. Pide el PIN si "cerrar caja" está protegida.
#[tauri::command]
pub async fn cerrar_turno(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
    efectivo_contado: i64,
    detalle_conteo: Option<Vec<Conteo>>,
    nota: Option<String>,
    pin: Option<String>,
) -> ResultadoProtegido<ResumenTurno> {
    let r = srv::cerrar(
        &estado.pool,
        &estado.pin,
        efectivo_contado,
        detalle_conteo.as_deref(),
        nota.as_deref(),
        pin.as_deref(),
    )
    .await?;
    super::avisar_cambios(&app);
    Ok(r)
}

#[tauri::command]
pub async fn listar_turnos(
    estado: State<'_, EstadoApp>,
    limite: i64,
    desplazamiento: i64,
) -> ResultadoComando<Vec<TurnoBreve>> {
    srv::listar(&estado.pool, limite, desplazamiento)
        .await
        .map_err(a_texto)
}

/// Imprime el "CIERRE DE CAJA" de un turno en la impresora configurada.
#[tauri::command]
pub async fn imprimir_cierre(estado: State<'_, EstadoApp>, turno_id: i64) -> ResultadoComando<()> {
    let trabajo = impresion::trabajo_cierre(&estado.pool, turno_id)
        .await
        .map_err(a_texto)?;
    super::impresion::enviar(&estado, trabajo).await
}

/// Guarda el PDF del cierre (cierre-000012.pdf) junto a los de las boletas.
#[tauri::command]
pub async fn guardar_cierre_pdf(
    app: AppHandle,
    estado: State<'_, EstadoApp>,
    turno_id: i64,
    abrir: bool,
) -> ResultadoComando<String> {
    let pendiente = impresion::preparar_pdf_cierre(&estado.pool, turno_id)
        .await
        .map_err(a_texto)?;
    super::boletas::guardar_pdf(&app, &estado, pendiente, abrir).await
}
