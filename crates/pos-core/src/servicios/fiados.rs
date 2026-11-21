//! Fiados: deudas de los clientes y cómo bajan.
//!
//! El saldo de una venta fiada es su total, menos lo que bajaron sus devoluciones y sus abonos
//! vigentes, más lo que se trasladó desde ella como saldo a favor. Cuando llega a cero pasa a
//! 'pagada' (con pagada_en). Una devolución nunca deja saldo negativo: lo que sobra pasa como
//! saldo a favor a las otras deudas del cliente (de la más antigua a la más nueva) y, si no
//! alcanza a usarse, se le devuelve en dinero.

use sqlx::SqliteConnection;

use crate::repositorios::fiados::{self as repo, NuevoAbono};
use crate::Resultado;

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
