//! Fiados: deudas de los clientes y cómo bajan.
//!
//! El saldo de una venta fiada es su total, menos lo que bajaron sus devoluciones y sus abonos
//! vigentes, más lo que se trasladó desde ella como saldo a favor. Cuando llega a cero pasa a
//! 'pagada' (con pagada_en). Una devolución nunca deja saldo negativo: lo que sobra pasa como
//! saldo a favor a las otras deudas del cliente (de la más antigua a la más nueva) y, si no
//! alcanza a usarse, se le devuelve en dinero.

use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

use crate::boleta::formato::{folio, pesos};
use crate::error::validacion;
use crate::repositorios::fiados::{self as repo, FilaFiada, FilaPago, NuevoAbono, NuevoPago};
use crate::repositorios::turnos as turnos_repo;
use crate::servicios::anulacion::motivo_valido;
use crate::servicios::clientes::{self, Cliente};
use crate::servicios::config;
use crate::servicios::proveedores::opcional;
use crate::servicios::seguridad::{self, Accion, ControlPin};
use crate::servicios::turnos::{self, MONTO_MAXIMO};
use crate::servicios::ventas::{redondeo_a_decena, MedioPago};
use crate::{Error, Resultado};

/// Reparte `monto` de saldo a favor (que viene de `origen`, una venta anulada o devuelta) entre
/// las otras deudas del cliente, de la más antigua a la más nueva. Devuelve cuánto se aplicó:
/// lo que falte para llegar a `monto` no tenía dónde aplicarse.
pub(crate) async fn trasladar_saldo_a_favor(
    conn: &mut SqliteConnection,
    cliente_id: i64,
    origen: i64,
    monto: i64,
    turno_id: Option<i64>,
) -> Resultado<i64> {
    if monto <= 0 {
        return Ok(0);
    }
    let medio = repo::medio_ultimo_abono(conn, origen)
        .await?
        .unwrap_or_else(|| "efectivo".to_owned());
    let mut resto = monto;
    for v in repo::pendientes(conn, cliente_id).await? {
        if resto == 0 {
            break;
        }
        if v.venta_id == origen || v.saldo <= 0 {
            continue;
        }
        let a = resto.min(v.saldo);
        repo::insertar_abono(
            conn,
            &NuevoAbono {
                venta_id: v.venta_id,
                monto: a,
                medio_pago: &medio,
                pago_id: None,
                traslado_de_venta_id: Some(origen),
                turno_id,
            },
        )
        .await?;
        if a == v.saldo {
            repo::marcar_pagada(conn, v.venta_id).await?;
        }
        resto -= a;
    }
    Ok(monto - resto)
}

/// Si la venta fiada quedó en cero (por abonos o devoluciones), pasa a 'pagada'.
pub(crate) async fn marcar_si_saldada(conn: &mut SqliteConnection, venta_id: i64) -> Resultado<()> {
    if repo::venta(conn, venta_id)
        .await?
        .is_some_and(|v| v.estado == "fiada" && v.saldo <= 0)
    {
        repo::marcar_pagada(conn, venta_id).await?;
    }
    Ok(())
}

// --- Deudores, ficha y pagos ---------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Deudor {
    pub id: i64,
    pub nombre: String,
    pub telefono: Option<String>,
    pub limite_credito: Option<i64>,
    pub activo: bool,
    pub deuda: i64,
    /// Fecha de la venta fiada pendiente más antigua.
    pub fiada_mas_antigua: Option<String>,
    pub ultimo_pago: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PaginaDeudores {
    pub clientes: Vec<Deudor>,
    /// Solo en la primera página: lo que se le debe al negocio y cuántos clientes deben.
    pub total_adeudado: Option<i64>,
    pub clientes_con_deuda: Option<i64>,
}

/// Clientes con deuda (o todos), de la mayor deuda a la menor.
pub async fn listar_deudores(
    pool: &SqlitePool,
    texto: &str,
    todos: bool,
    limite: i64,
    desplazamiento: i64,
) -> Resultado<PaginaDeudores> {
    let texto = texto.trim();
    let mut conn = pool.acquire().await?;
    let desplazamiento = desplazamiento.max(0);
    let clientes = repo::deudores(
        &mut conn,
        (!texto.is_empty()).then_some(texto),
        todos,
        limite.clamp(1, 200),
        desplazamiento,
    )
    .await?
    .into_iter()
    .map(|f| Deudor {
        id: f.id,
        nombre: f.nombre,
        telefono: f.telefono,
        limite_credito: f.limite_credito,
        activo: f.activo,
        deuda: f.deuda,
        fiada_mas_antigua: f.fiada_mas_antigua,
        ultimo_pago: f.ultimo_pago,
    })
    .collect();
    let (total, con_deuda) = if desplazamiento == 0 {
        let (t, c) = repo::total_adeudado(&mut conn).await?;
        (Some(t), Some(c))
    } else {
        (None, None)
    };
    Ok(PaginaDeudores {
        clientes,
        total_adeudado: total,
        clientes_con_deuda: con_deuda,
    })
}

/// Una venta fiada con lo que se debe de ella.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VentaFiada {
    pub venta_id: i64,
    pub folio: i64,
    pub fecha_hora: String,
    pub total: i64,
    /// Lo que bajaron sus devoluciones.
    pub devuelto: i64,
    pub abonado: i64,
    pub saldo: i64,
    pub pagada_en: Option<String>,
}

impl From<FilaFiada> for VentaFiada {
    fn from(f: FilaFiada) -> Self {
        VentaFiada {
            venta_id: f.venta_id,
            folio: f.folio,
            fecha_hora: f.fecha_hora,
            total: f.total,
            devuelto: f.devuelto,
            abonado: f.abonado,
            saldo: f.saldo.max(0),
            pagada_en: f.pagada_en,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AbonoDePago {
    pub venta_id: i64,
    pub folio: i64,
    pub monto: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Pago {
    pub id: i64,
    pub cliente_id: i64,
    pub fecha: String,
    /// Lo que bajó la deuda; en efectivo entró monto + redondeo.
    pub monto: i64,
    pub redondeo: i64,
    pub monto_recibido: Option<i64>,
    pub vuelto: Option<i64>,
    pub medio_pago: MedioPago,
    pub turno_id: Option<i64>,
    pub nota: Option<String>,
    pub anulado_en: Option<String>,
    pub motivo_anulacion: Option<String>,
    pub abonos: Vec<AbonoDePago>,
}

async fn pago_completo(conn: &mut SqliteConnection, f: FilaPago) -> Resultado<Pago> {
    let abonos = repo::abonos_de_pago(conn, f.id)
        .await?
        .into_iter()
        .map(|a| AbonoDePago {
            venta_id: a.venta_id,
            folio: a.folio,
            monto: a.monto,
        })
        .collect();
    Ok(Pago {
        medio_pago: MedioPago::desde_texto(&f.medio_pago).unwrap_or(MedioPago::Efectivo),
        id: f.id,
        cliente_id: f.cliente_id,
        fecha: f.fecha,
        monto: f.monto,
        redondeo: f.redondeo,
        monto_recibido: f.monto_recibido,
        vuelto: f.vuelto,
        turno_id: f.turno_id,
        nota: f.nota,
        anulado_en: f.anulado_en,
        motivo_anulacion: f.motivo_anulacion,
        abonos,
    })
}

/// Un pago con sus abonos (para imprimir su comprobante).
pub async fn pago(pool: &SqlitePool, pago_id: i64) -> Resultado<Pago> {
    let mut conn = pool.acquire().await?;
    let fila = repo::pago(&mut conn, pago_id)
        .await?
        .ok_or_else(|| Error::NoEncontrado("El pago no existe".into()))?;
    pago_completo(&mut conn, fila).await
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FichaCliente {
    pub cliente: Cliente,
    /// De la más antigua a la más nueva (así se pagan).
    pub pendientes: Vec<VentaFiada>,
    /// Las últimas 50 ya saldadas.
    pub pagadas: Vec<VentaFiada>,
    /// Los últimos 50 pagos, también los anulados.
    pub pagos: Vec<Pago>,
}

pub async fn ficha(pool: &SqlitePool, cliente_id: i64) -> Resultado<FichaCliente> {
    let cliente = clientes::obtener(pool, cliente_id).await?;
    let mut conn = pool.acquire().await?;
    let pendientes = repo::pendientes(&mut conn, cliente_id)
        .await?
        .into_iter()
        .map(VentaFiada::from)
        .collect();
    let pagadas = repo::pagadas(&mut conn, cliente_id, 50)
        .await?
        .into_iter()
        .map(VentaFiada::from)
        .collect();
    let mut pagos = Vec::new();
    for f in repo::pagos(&mut conn, cliente_id, 50).await? {
        pagos.push(pago_completo(&mut conn, f).await?);
    }
    Ok(FichaCliente {
        cliente,
        pendientes,
        pagadas,
        pagos,
    })
}

/// Lo que el cliente entrega. Sin `ventas`, se aplica de la venta más antigua a la más nueva;
/// con `ventas`, primero a esas (las que se marcaron para saldar) y lo que sobre, al resto.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuevoPagoCliente {
    pub cliente_id: i64,
    pub monto: i64,
    pub medio_pago: MedioPago,
    #[serde(default)]
    pub ventas: Vec<i64>,
    /// Solo en efectivo: lo que entregó (vacío = justo).
    pub monto_recibido: Option<i64>,
    pub nota: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PagoRegistrado {
    pub pago: Pago,
    /// Lo que debe después del pago (o de su anulación).
    pub deuda_restante: i64,
}

/// Registra el pago en una transacción: lo reparte en abonos por venta, deja como 'pagada' cada
/// venta que llega a cero y queda en el turno (en efectivo exige la caja abierta y se redondea a
/// la decena, como al cobrar). No puede superar la deuda.
pub async fn registrar_pago(pool: &SqlitePool, p: &NuevoPagoCliente) -> Resultado<PagoRegistrado> {
    if p.monto <= 0 || p.monto > MONTO_MAXIMO {
        return validacion("El monto del pago debe ser mayor que $0");
    }
    if p.medio_pago == MedioPago::Fiado {
        return validacion("Un pago no puede ser fiado");
    }
    let nota = opcional(p.nota.as_deref(), 200, "La nota")?;
    let efectivo = p.medio_pago == MedioPago::Efectivo;

    let mut tx = pool.begin().await?;
    if crate::repositorios::clientes::por_id(&mut tx, p.cliente_id)
        .await?
        .is_none()
    {
        return Err(Error::NoEncontrado("El cliente no existe".into()));
    }
    let turno_id = turnos_repo::id_abierto(&mut tx).await?;
    if efectivo && turno_id.is_none() {
        return Err(turnos::sin_caja("recibir pagos en efectivo"));
    }
    let pendientes = repo::pendientes(&mut tx, p.cliente_id).await?;
    let deuda: i64 = pendientes.iter().map(|v| v.saldo.max(0)).sum();
    if p.monto > deuda {
        return validacion(format!(
            "El pago ({}) es mayor que la deuda ({})",
            pesos(p.monto),
            pesos(deuda)
        ));
    }
    if p.ventas
        .iter()
        .any(|id| !pendientes.iter().any(|v| v.venta_id == *id))
    {
        return validacion("Una de las ventas elegidas no es una deuda de este cliente");
    }
    let (redondeo, monto_recibido, vuelto) = if efectivo {
        let redondeo = if config::redondeo_efectivo_activo(&mut tx).await? {
            redondeo_a_decena(p.monto)
        } else {
            0
        };
        let entra = p.monto + redondeo;
        let recibido = p.monto_recibido.unwrap_or(entra);
        if recibido < entra {
            return validacion("El monto recibido es menor que el pago");
        }
        (redondeo, Some(recibido), Some(recibido - entra))
    } else {
        (0, None, None)
    };

    let pago_id = repo::insertar_pago(
        &mut tx,
        &NuevoPago {
            cliente_id: p.cliente_id,
            monto: p.monto,
            redondeo,
            monto_recibido,
            vuelto,
            medio_pago: p.medio_pago.como_texto(),
            turno_id,
            nota: nota.as_deref(),
        },
    )
    .await?;
    // Primero las elegidas, después el resto; en cada grupo, de la más antigua a la más nueva.
    let mut orden: Vec<&FilaFiada> = pendientes
        .iter()
        .filter(|v| p.ventas.contains(&v.venta_id))
        .collect();
    orden.extend(
        pendientes
            .iter()
            .filter(|v| !p.ventas.contains(&v.venta_id)),
    );
    let mut resto = p.monto;
    for v in orden {
        if resto == 0 {
            break;
        }
        if v.saldo <= 0 {
            continue;
        }
        let a = resto.min(v.saldo);
        repo::insertar_abono(
            &mut tx,
            &NuevoAbono {
                venta_id: v.venta_id,
                monto: a,
                medio_pago: p.medio_pago.como_texto(),
                pago_id: Some(pago_id),
                traslado_de_venta_id: None,
                turno_id,
            },
        )
        .await?;
        if a == v.saldo {
            repo::marcar_pagada(&mut tx, v.venta_id).await?;
        }
        resto -= a;
    }
    let deuda_restante = repo::deuda(&mut tx, p.cliente_id).await?;
    let fila = repo::pago(&mut tx, pago_id)
        .await?
        .expect("recién insertado");
    let pago = pago_completo(&mut tx, fila).await?;
    tx.commit().await?;
    Ok(PagoRegistrado {
        pago,
        deuda_restante,
    })
}

/// Anula un pago mal ingresado (PIN de anular venta): sus abonos dejan de contar, las ventas que
/// vuelven a deber pasan a 'fiada' y, si fue en efectivo, el dinero sale del turno actual. No se
/// anula si alguna venta que pagó se anuló o devolvió dinero después: descuadraría la caja.
pub async fn anular_pago(
    pool: &SqlitePool,
    control: &ControlPin,
    pago_id: i64,
    motivo: &str,
    pin: Option<&str>,
) -> Resultado<PagoRegistrado> {
    let motivo = motivo_valido(motivo, "anulación")?;
    let (fila, abonos) = {
        let mut conn = pool.acquire().await?;
        let fila = repo::pago(&mut conn, pago_id)
            .await?
            .ok_or_else(|| Error::NoEncontrado("El pago no existe".into()))?;
        let abonos = repo::abonos_de_pago(&mut conn, pago_id).await?;
        (fila, abonos)
    };
    if fila.anulado_en.is_some() {
        return validacion("El pago ya está anulado");
    }
    if let Some(a) = abonos.iter().find(|a| a.bloquea) {
        return validacion(format!(
            "La venta N° {} que pagó se anuló o devolvió dinero después: el pago ya no se puede anular",
            folio(a.folio)
        ));
    }
    let efectivo = fila.medio_pago == "efectivo";
    if efectivo && turnos::estado_caja(pool).await?.turno.is_none() {
        return Err(turnos::sin_caja("devolver un pago en efectivo"));
    }
    seguridad::exigir(pool, control, Accion::AnularVenta, pin).await?;

    let mut tx = pool.begin().await?;
    let turno_id = turnos_repo::id_abierto(&mut tx).await?;
    if efectivo && turno_id.is_none() {
        return Err(turnos::sin_caja("devolver un pago en efectivo"));
    }
    if !repo::anular_pago(&mut tx, pago_id, &motivo, turno_id).await? {
        return validacion("El pago ya está anulado");
    }
    for a in &abonos {
        if a.estado == "pagada"
            && repo::venta(&mut tx, a.venta_id)
                .await?
                .is_some_and(|v| v.saldo > 0)
        {
            repo::marcar_fiada(&mut tx, a.venta_id).await?;
        }
    }
    let deuda_restante = repo::deuda(&mut tx, fila.cliente_id).await?;
    let fila = repo::pago(&mut tx, pago_id).await?.expect("existe");
    let pago = pago_completo(&mut tx, fila).await?;
    tx.commit().await?;
    Ok(PagoRegistrado {
        pago,
        deuda_restante,
    })
}
