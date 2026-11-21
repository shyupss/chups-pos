//! Turnos de caja: abrir con un monto inicial, ingresos y retiros de efectivo, y cerrar contando
//! el efectivo contra lo que debería haber.
//!
//! Efectivo esperado = monto inicial + ventas en efectivo + abonos en efectivo + ingresos
//! − retiros − reembolsos en efectivo (anulaciones y devoluciones hechas en el turno).
//!
//! Las ventas cuentan en el turno en que se cobraron, aunque después se anulen: ese dinero sí
//! entró. La anulación o devolución cuenta en el turno en que se hizo. Así el resumen de un turno
//! cerrado nunca cambia.

use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

use crate::boleta::formato::pesos;
use crate::error::validacion;
use crate::repositorios::turnos::{self as repo, FilaMovimiento, FilaTurno, PorMedio};
use crate::servicios::seguridad::{self, Accion, ControlPin};
use crate::servicios::ventas::MedioPago;
use crate::{Error, Resultado};

/// Tope para cualquier monto de la caja: cien millones de pesos.
pub const MONTO_MAXIMO: i64 = 100_000_000;

/// Billetes y monedas chilenos en circulación, de mayor a menor.
pub const DENOMINACIONES: [i64; 9] = [20_000, 10_000, 5_000, 2_000, 1_000, 500, 100, 50, 10];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoMovimiento {
    Ingreso,
    Retiro,
}

impl TipoMovimiento {
    pub fn como_texto(self) -> &'static str {
        match self {
            TipoMovimiento::Ingreso => "ingreso",
            TipoMovimiento::Retiro => "retiro",
        }
    }
}

/// Cuántos billetes o monedas de una denominación se contaron.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conteo {
    pub denominacion: i64,
    pub cantidad: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Turno {
    pub id: i64,
    pub abierto_en: String,
    pub cerrado_en: Option<String>,
    pub abierto: bool,
    pub monto_inicial: i64,
    pub efectivo_esperado: Option<i64>,
    pub efectivo_contado: Option<i64>,
    /// Contado − esperado: positivo sobra, negativo falta.
    pub diferencia: Option<i64>,
    pub detalle_conteo: Option<Vec<Conteo>>,
    pub nota_cierre: Option<String>,
}

impl From<FilaTurno> for Turno {
    fn from(f: FilaTurno) -> Self {
        Turno {
            id: f.id,
            abierto: f.estado == "abierto",
            abierto_en: f.abierto_en,
            cerrado_en: f.cerrado_en,
            monto_inicial: f.monto_inicial,
            efectivo_esperado: f.efectivo_esperado,
            efectivo_contado: f.efectivo_contado,
            diferencia: f.diferencia,
            // Un JSON dañado no impide ver el turno: queda sin detalle.
            detalle_conteo: f
                .detalle_conteo
                .as_deref()
                .and_then(|j| serde_json::from_str(j).ok()),
            nota_cierre: f.nota_cierre,
        }
    }
}

/// Lo que necesita la Caja al abrir: el turno abierto o, si no hay, qué monto sugerir.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EstadoCaja {
    pub turno: Option<Turno>,
    pub ultimo_monto_inicial: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MovimientoCaja {
    pub id: i64,
    pub tipo: TipoMovimiento,
    pub monto: i64,
    pub motivo: String,
    pub fecha: String,
}

impl From<FilaMovimiento> for MovimientoCaja {
    fn from(f: FilaMovimiento) -> Self {
        MovimientoCaja {
            id: f.id,
            tipo: if f.tipo == "retiro" {
                TipoMovimiento::Retiro
            } else {
                TipoMovimiento::Ingreso
            },
            monto: f.monto,
            motivo: f.motivo,
            fecha: f.fecha,
        }
    }
}

/// Cantidad y monto de un medio de pago (o de reembolso: `Fiado` = sin reembolso, baja la deuda).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TotalMedio {
    pub medio: MedioPago,
    pub cantidad: i64,
    pub monto: i64,
    /// Ajuste a la decena incluido en `monto`.
    pub redondeo: i64,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CuadreEfectivo {
    pub monto_inicial: i64,
    pub ventas: i64,
    pub abonos: i64,
    pub ingresos: i64,
    pub retiros: i64,
    /// Lo que salió en efectivo por anulaciones y devoluciones.
    pub reembolsos: i64,
    pub esperado: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResumenTurno {
    pub turno: Turno,
    pub cantidad_ventas: i64,
    pub total_ventas: i64,
    /// Un renglón por medio de pago, aunque sea cero, en el orden de la pantalla de cobro.
    pub ventas: Vec<TotalMedio>,
    /// Ajuste a la decena de las ventas en efectivo (positivo: el cliente pagó de más).
    pub redondeo_ventas: i64,
    pub abonos: Vec<TotalMedio>,
    pub anulaciones: Vec<TotalMedio>,
    pub devoluciones: Vec<TotalMedio>,
    /// Ajuste a la decena de los reembolsos en efectivo (negativo: se devolvió de menos).
    pub redondeo_reembolsos: i64,
    pub movimientos: Vec<MovimientoCaja>,
    pub efectivo: CuadreEfectivo,
}

const MEDIOS: [MedioPago; 5] = [
    MedioPago::Efectivo,
    MedioPago::Debito,
    MedioPago::Credito,
    MedioPago::Transferencia,
    MedioPago::Fiado,
];

fn totales(filas: Vec<PorMedio>) -> Vec<TotalMedio> {
    let mut t: Vec<TotalMedio> = filas
        .into_iter()
        .filter_map(|f| {
            Some(TotalMedio {
                medio: MedioPago::desde_texto(&f.medio)?,
                cantidad: f.cantidad,
                monto: f.monto,
                redondeo: f.redondeo,
            })
        })
        .collect();
    t.sort_by_key(|x| MEDIOS.iter().position(|m| *m == x.medio));
    t
}

fn de(t: &[TotalMedio], medio: MedioPago) -> TotalMedio {
    t.iter()
        .find(|x| x.medio == medio)
        .copied()
        .unwrap_or(TotalMedio {
            medio,
            cantidad: 0,
            monto: 0,
            redondeo: 0,
        })
}

/// Junta todo lo del turno y calcula el efectivo esperado.
async fn armar_resumen(conn: &mut SqliteConnection, fila: FilaTurno) -> Resultado<ResumenTurno> {
    let id = fila.id;
    let vendidas = totales(repo::ventas_por_medio(conn, id).await?);
    let abonos = totales(repo::abonos_por_medio(conn, id).await?);
    let anulaciones = totales(repo::anulaciones_por_medio(conn, id).await?);
    let devoluciones = totales(repo::devoluciones_por_medio(conn, id).await?);
    let movimientos: Vec<MovimientoCaja> = repo::movimientos(conn, id)
        .await?
        .into_iter()
        .map(MovimientoCaja::from)
        .collect();

    let suma = |tipo: TipoMovimiento| {
        movimientos
            .iter()
            .filter(|m| m.tipo == tipo)
            .map(|m| m.monto)
            .sum::<i64>()
    };
    let efectivo = MedioPago::Efectivo;
    let reembolsos = de(&anulaciones, efectivo).monto + de(&devoluciones, efectivo).monto;
    let mut cuadre = CuadreEfectivo {
        monto_inicial: fila.monto_inicial,
        ventas: de(&vendidas, efectivo).monto,
        abonos: de(&abonos, efectivo).monto,
        ingresos: suma(TipoMovimiento::Ingreso),
        retiros: suma(TipoMovimiento::Retiro),
        reembolsos,
        esperado: 0,
    };
    cuadre.esperado = cuadre.monto_inicial + cuadre.ventas + cuadre.abonos + cuadre.ingresos
        - cuadre.retiros
        - cuadre.reembolsos;

    Ok(ResumenTurno {
        cantidad_ventas: vendidas.iter().map(|t| t.cantidad).sum(),
        total_ventas: vendidas.iter().map(|t| t.monto).sum(),
        redondeo_ventas: vendidas.iter().map(|t| t.redondeo).sum(),
        ventas: MEDIOS.iter().map(|m| de(&vendidas, *m)).collect(),
        redondeo_reembolsos: anulaciones
            .iter()
            .chain(&devoluciones)
            .map(|t| t.redondeo)
            .sum(),
        abonos,
        anulaciones,
        devoluciones,
        movimientos,
        efectivo: cuadre,
        turno: Turno::from(fila),
    })
}

fn validar_monto(monto: i64, que: &str) -> Resultado<()> {
    if !(0..=MONTO_MAXIMO).contains(&monto) {
        return validacion(format!(
            "{que} debe estar entre $0 y {}",
            pesos(MONTO_MAXIMO)
        ));
    }
    Ok(())
}

/// Texto obligatorio u opcional: sin espacios sobrantes y con largo máximo.
fn texto(t: Option<&str>, maximo: usize, que: &str) -> Resultado<Option<String>> {
    let t = t.map(str::trim).filter(|t| !t.is_empty());
    if t.is_some_and(|t| t.chars().count() > maximo) {
        return validacion(format!("{que} no puede pasar de {maximo} caracteres"));
    }
    Ok(t.map(str::to_owned))
}

pub async fn estado_caja(pool: &SqlitePool) -> Resultado<EstadoCaja> {
    let mut conn = pool.acquire().await?;
    let turno = repo::abierto(&mut conn).await?.map(Turno::from);
    let ultimo_monto_inicial = repo::ultimo_monto_inicial(&mut conn).await?;
    Ok(EstadoCaja {
        turno,
        ultimo_monto_inicial,
    })
}

/// Abre la caja con el efectivo que hay al empezar.
pub async fn abrir(pool: &SqlitePool, monto_inicial: i64) -> Resultado<Turno> {
    validar_monto(monto_inicial, "El monto inicial")?;
    let mut conn = pool.acquire().await?;
    Ok(repo::insertar(&mut conn, monto_inicial).await?.into())
}

/// Error para lo que necesita la caja abierta.
fn sin_caja(que: &str) -> Error {
    Error::Validacion(format!("Abre la caja para {que}"))
}

/// Ingreso o retiro de efectivo con su motivo. El retiro pide el PIN si está protegido y no
/// puede sacar más de lo que debería haber en la caja.
pub async fn registrar_movimiento(
    pool: &SqlitePool,
    control: &ControlPin,
    tipo: TipoMovimiento,
    monto: i64,
    motivo: &str,
    pin: Option<&str>,
) -> Resultado<MovimientoCaja> {
    if monto <= 0 || monto > MONTO_MAXIMO {
        return validacion(format!(
            "El monto debe ser mayor que $0 y hasta {}",
            pesos(MONTO_MAXIMO)
        ));
    }
    let Some(motivo) = texto(Some(motivo), 200, "El motivo")? else {
        return validacion("Escribe el motivo del movimiento");
    };
    // Primero si hay caja, para no pedir el PIN en vano.
    if estado_caja(pool).await?.turno.is_none() {
        return Err(sin_caja("registrar ingresos o retiros"));
    }
    if tipo == TipoMovimiento::Retiro {
        seguridad::exigir(pool, control, Accion::RetiroEfectivo, pin).await?;
    }

    let mut tx = pool.begin().await?;
    let fila = repo::abierto(&mut tx)
        .await?
        .ok_or_else(|| sin_caja("registrar ingresos o retiros"))?;
    let turno_id = fila.id;
    if tipo == TipoMovimiento::Retiro {
        let esperado = armar_resumen(&mut tx, fila).await?.efectivo.esperado;
        if monto > esperado {
            return validacion(format!(
                "En la caja debería haber {}: no se puede retirar más que eso",
                pesos(esperado.max(0))
            ));
        }
    }
    let m = repo::insertar_movimiento(&mut tx, turno_id, tipo.como_texto(), monto, &motivo).await?;
    tx.commit().await?;
    Ok(m.into())
}

pub async fn resumen(pool: &SqlitePool, turno_id: i64) -> Resultado<ResumenTurno> {
    let mut conn = pool.acquire().await?;
    let fila = repo::por_id(&mut conn, turno_id)
        .await?
        .ok_or_else(|| Error::NoEncontrado("El turno no existe".into()))?;
    armar_resumen(&mut conn, fila).await
}

/// Revisa el conteo por denominación y lo deja ordenado de mayor a menor, sin ceros.
fn validar_conteo(detalle: &[Conteo], contado: i64) -> Resultado<Vec<Conteo>> {
    let mut limpio = Vec::new();
    for d in DENOMINACIONES {
        let mut del_tipo = detalle.iter().filter(|c| c.denominacion == d);
        let cantidad = match (del_tipo.next(), del_tipo.next()) {
            (None, _) => 0,
            (Some(c), None) => c.cantidad,
            (Some(_), Some(_)) => return validacion("Cada billete o moneda va una sola vez"),
        };
        if !(0..=100_000).contains(&cantidad) {
            return validacion("La cantidad de billetes o monedas debe estar entre 0 y 100.000");
        }
        if cantidad > 0 {
            limpio.push(Conteo {
                denominacion: d,
                cantidad,
            });
        }
    }
    if let Some(c) = detalle
        .iter()
        .find(|c| !DENOMINACIONES.contains(&c.denominacion))
    {
        return validacion(format!(
            "No existe la denominación de {}",
            pesos(c.denominacion)
        ));
    }
    let suma: i64 = limpio.iter().map(|c| c.denominacion * c.cantidad).sum();
    if suma != contado {
        return validacion(format!(
            "El conteo suma {} pero el efectivo contado es {}",
            pesos(suma),
            pesos(contado)
        ));
    }
    Ok(limpio)
}

/// Cierra el turno abierto con el efectivo contado (y, si se contó por denominación, el detalle).
/// Pide el PIN si "cerrar caja" está protegida. Devuelve el resumen con la diferencia.
pub async fn cerrar(
    pool: &SqlitePool,
    control: &ControlPin,
    efectivo_contado: i64,
    detalle_conteo: Option<&[Conteo]>,
    nota: Option<&str>,
    pin: Option<&str>,
) -> Resultado<ResumenTurno> {
    validar_monto(efectivo_contado, "El efectivo contado")?;
    let detalle = detalle_conteo
        .map(|d| validar_conteo(d, efectivo_contado))
        .transpose()?;
    let nota = texto(nota, 300, "La nota")?;
    if estado_caja(pool).await?.turno.is_none() {
        return validacion("La caja ya está cerrada");
    }
    seguridad::exigir(pool, control, Accion::CerrarCaja, pin).await?;

    let mut tx = pool.begin().await?;
    let fila = repo::abierto(&mut tx)
        .await?
        .ok_or_else(|| Error::Validacion("La caja ya está cerrada".into()))?;
    let id = fila.id;
    let esperado = armar_resumen(&mut tx, fila).await?.efectivo.esperado;
    let json = detalle
        .map(|d| serde_json::to_string(&d))
        .transpose()
        .map_err(|e| Error::Validacion(format!("No se pudo guardar el conteo: {e}")))?;
    let cerrado = repo::cerrar(
        &mut tx,
        id,
        &repo::Cierre {
            esperado,
            contado: efectivo_contado,
            detalle_conteo: json.as_deref(),
            nota: nota.as_deref(),
        },
    )
    .await?;
    if !cerrado {
        return validacion("La caja ya está cerrada");
    }
    tx.commit().await?;
    resumen(pool, id).await
}

/// Turno abreviado para elegirlo en un filtro.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TurnoBreve {
    pub id: i64,
    pub abierto_en: String,
    pub cerrado_en: Option<String>,
    pub abierto: bool,
}

pub async fn listar(
    pool: &SqlitePool,
    limite: i64,
    desplazamiento: i64,
) -> Resultado<Vec<TurnoBreve>> {
    Ok(
        repo::listar(pool, limite.clamp(1, 200), desplazamiento.max(0))
            .await?
            .into_iter()
            .map(|f| TurnoBreve {
                abierto: f.estado == "abierto",
                id: f.id,
                abierto_en: f.abierto_en,
                cerrado_en: f.cerrado_en,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valida_el_conteo() {
        let c = |denominacion, cantidad| Conteo {
            denominacion,
            cantidad,
        };
        let limpio = validar_conteo(&[c(500, 4), c(20_000, 2), c(10, 0)], 42_000).unwrap();
        assert_eq!(limpio, [c(20_000, 2), c(500, 4)]);
        assert!(validar_conteo(&[c(20_000, 2)], 41_000).is_err());
        assert!(validar_conteo(&[c(300, 1)], 300).is_err());
        assert!(validar_conteo(&[c(500, 1), c(500, 1)], 1000).is_err());
        assert!(validar_conteo(&[c(500, -1)], -500).is_err());
        assert_eq!(validar_conteo(&[], 0).unwrap(), []);
    }
}
