//! Reportes: la pantalla Hoy y los reportes por rango (ventas, productos, ganancia, turnos y
//! fiados). Cada uno es una sola llamada que lee lo justo con los índices por fecha y arma en
//! Rust los días y horas locales (ver `zona`).
//!
//! Reglas comunes: las ventas anuladas no cuentan; las devoluciones se descuentan de la venta a
//! la que pertenecen (así el total de un rango cuadra con el historial de Ventas); la ganancia
//! solo se calcula sobre líneas con costo y aparte se informa lo que no lo tiene.

use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Datelike, Duration, NaiveDate, Timelike, Utc};
use serde::Serialize;
use sqlx::{SqliteConnection, SqlitePool};

use crate::error::validacion;
use crate::repositorios::productos::Unidad;
use crate::repositorios::reportes::{self as repo, FilaHora, FilaProducto};
use crate::servicios::ventas::MedioPago;
use crate::servicios::{config, turnos};
use crate::zona::{self, Zona};
use crate::Resultado;

/// Rango de días locales, ambos incluidos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rango {
    pub desde: NaiveDate,
    pub hasta: NaiveDate,
}

impl Rango {
    /// Valida "aaaa-mm-dd" y que no sea al revés ni de más de 10 años.
    pub fn de_textos(desde: &str, hasta: &str) -> Resultado<Rango> {
        let desde = crate::servicios::historial::dia(desde)?;
        let hasta = crate::servicios::historial::dia(hasta)?;
        if desde > hasta {
            return validacion("La fecha de inicio es posterior a la de término");
        }
        if hasta - desde > Duration::days(3660) {
            return validacion("El rango no puede pasar de 10 años");
        }
        Ok(Rango { desde, hasta })
    }

    fn limites(&self, zona: &dyn Zona) -> (String, String) {
        zona::limites(zona, self.desde, self.hasta)
    }

    fn dias(&self) -> impl Iterator<Item = NaiveDate> {
        let hasta = self.hasta;
        self.desde.iter_days().take_while(move |d| *d <= hasta)
    }
}

/// Porcentaje en puntos básicos (1 = 0,01 %), para no mandar decimales: 1234 = 12,34 %.
fn puntos_basicos(parte: i64, total: i64) -> Option<i64> {
    (total != 0).then(|| {
        let p = parte as i128 * 10_000;
        let t = total as i128;
        // Redondeo al más cercano, simétrico para negativos.
        ((p + p.signum() * t.abs() / 2) / t) as i64
    })
}

fn promedio(monto: i64, cantidad: i64) -> i64 {
    if cantidad == 0 {
        0
    } else {
        (monto + cantidad / 2) / cantidad
    }
}

// --- Ventas agrupadas por hora local ------------------------------------------------------

/// Las filas por hora UTC traducidas a hora local.
struct HoraLocal {
    local: chrono::NaiveDateTime,
    medio: MedioPago,
    boletas: i64,
    monto: i64,
}

fn a_local(zona: &dyn Zona, filas: Vec<FilaHora>) -> Vec<HoraLocal> {
    filas
        .into_iter()
        .filter_map(|f| {
            Some(HoraLocal {
                local: zona.a_local(zona::de_hora_utc(&f.hora)?),
                medio: MedioPago::desde_texto(&f.medio_pago)?,
                boletas: f.boletas,
                monto: f.monto,
            })
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Celda {
    pub monto: i64,
    pub boletas: i64,
}

impl Celda {
    fn sumar(&mut self, monto: i64, boletas: i64) {
        self.monto += monto;
        self.boletas += boletas;
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TotalMedio {
    pub medio: MedioPago,
    pub monto: i64,
    pub boletas: i64,
}

const MEDIOS: [MedioPago; 5] = [
    MedioPago::Efectivo,
    MedioPago::Debito,
    MedioPago::Credito,
    MedioPago::Transferencia,
    MedioPago::Fiado,
];

fn por_medio(horas: &[HoraLocal]) -> Vec<TotalMedio> {
    MEDIOS
        .iter()
        .filter_map(|m| {
            let mut c = Celda::default();
            for h in horas.iter().filter(|h| h.medio == *m) {
                c.sumar(h.monto, h.boletas);
            }
            (c.boletas > 0).then_some(TotalMedio {
                medio: *m,
                monto: c.monto,
                boletas: c.boletas,
            })
        })
        .collect()
}

// --- Productos: vendido menos devuelto -----------------------------------------------------

/// Lo neto de un producto en el rango.
#[derive(Debug, Clone, Default)]
struct Neto {
    nombre: String,
    unidad: String,
    categoria_id: Option<i64>,
    cantidad_mil: i64,
    monto: i64,
    monto_con_costo: i64,
    costo: i64,
    monto_estimado: i64,
    costo_estimado: i64,
}

async fn netos(
    conn: &mut SqliteConnection,
    inicio: &str,
    fin: &str,
) -> Resultado<BTreeMap<i64, Neto>> {
    let mut netos: BTreeMap<i64, Neto> = BTreeMap::new();
    for (f, signo) in repo::vendido_por_producto(conn, inicio, fin)
        .await?
        .into_iter()
        .map(|f| (f, 1))
        .chain(
            repo::devuelto_por_producto(conn, inicio, fin)
                .await?
                .into_iter()
                .map(|f| (f, -1)),
        )
    {
        let FilaProducto {
            producto_id,
            nombre,
            unidad,
            categoria_id,
            cantidad_mil,
            monto,
            monto_con_costo,
            costo,
            monto_estimado,
            costo_estimado,
        } = f;
        let n = netos.entry(producto_id).or_insert_with(|| Neto {
            nombre,
            unidad,
            categoria_id,
            ..Default::default()
        });
        n.cantidad_mil += signo * cantidad_mil;
        n.monto += signo * monto;
        n.monto_con_costo += signo * monto_con_costo;
        n.costo += signo * costo;
        n.monto_estimado += signo * monto_estimado;
        n.costo_estimado += signo * costo_estimado;
    }
    // Lo devuelto entero no es una venta.
    netos.retain(|_, n| n.cantidad_mil != 0 || n.monto != 0);
    Ok(netos)
}

/// Un producto en un ranking o en la ganancia.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProductoVendido {
    pub producto_id: i64,
    pub nombre: String,
    pub unidad: Unidad,
    pub categoria_id: Option<i64>,
    pub categoria: Option<String>,
    pub cantidad_mil: i64,
    /// Lo vendido, ya sin lo devuelto.
    pub monto: i64,
    /// Lo vendido de líneas con costo, su costo y la ganancia (`None` si ninguna línea tiene).
    pub monto_con_costo: i64,
    pub costo: Option<i64>,
    pub ganancia: Option<i64>,
    pub margen_pb: Option<i64>,
    /// Lo vendido sin costo conocido: no entra en la ganancia.
    pub monto_sin_costo: i64,
    /// Parte del costo es estimada (ventas de antes de guardar el costo en cada línea).
    pub costo_estimado: bool,
}

fn producto(id: i64, n: &Neto, categorias: &HashMap<i64, String>) -> ProductoVendido {
    let tiene_costo = n.monto_con_costo != 0 || n.costo != 0;
    let ganancia = tiene_costo.then_some(n.monto_con_costo - n.costo);
    ProductoVendido {
        producto_id: id,
        nombre: n.nombre.clone(),
        unidad: if n.unidad == "kg" {
            Unidad::Kg
        } else {
            Unidad::Unidad
        },
        categoria_id: n.categoria_id,
        categoria: n.categoria_id.and_then(|c| categorias.get(&c).cloned()),
        cantidad_mil: n.cantidad_mil,
        monto: n.monto,
        monto_con_costo: n.monto_con_costo,
        costo: tiene_costo.then_some(n.costo),
        ganancia,
        margen_pb: ganancia.and_then(|g| puntos_basicos(g, n.monto_con_costo)),
        monto_sin_costo: n.monto - n.monto_con_costo,
        costo_estimado: n.monto_estimado != 0 || n.costo_estimado != 0,
    }
}

async fn nombres_categorias(conn: &mut SqliteConnection) -> Resultado<HashMap<i64, String>> {
    Ok(repo::categorias(conn).await?.into_iter().collect())
}

fn productos(
    netos: &BTreeMap<i64, Neto>,
    categorias: &HashMap<i64, String>,
) -> Vec<ProductoVendido> {
    let mut v: Vec<ProductoVendido> = netos
        .iter()
        .map(|(id, n)| producto(*id, n, categorias))
        .collect();
    v.sort_by(|a, b| b.monto.cmp(&a.monto).then_with(|| a.nombre.cmp(&b.nombre)));
    v
}

// --- Pantalla Hoy -------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BajoMinimo {
    pub id: i64,
    pub nombre: String,
    pub unidad: Unidad,
    pub stock_mil: i64,
    pub stock_minimo_mil: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClienteAtrasado {
    pub id: i64,
    pub nombre: String,
    pub deuda: i64,
    /// Fecha (UTC) de su fiada pendiente más antigua y cuántos días tiene.
    pub desde: String,
    pub dias: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResumenHoy {
    /// Día local "aaaa-mm-dd".
    pub fecha: String,
    pub ventas: i64,
    pub boletas: i64,
    pub ticket_promedio: i64,
    /// Ganancia de lo vendido con costo; lo vendido sin costo va aparte.
    pub ganancia: i64,
    pub ventas_sin_costo: i64,
    /// Ventas del mismo día de la semana anterior hasta la misma hora, y la variación.
    pub ventas_semana_anterior: i64,
    pub variacion_pb: Option<i64>,
    /// Efectivo esperado del turno abierto; `None` con la caja cerrada.
    pub efectivo_esperado: Option<i64>,
    pub fiado_otorgado: i64,
    pub pagos_fiado: i64,
    /// Las 24 horas del día.
    pub por_hora: Vec<Celda>,
    pub por_medio: Vec<TotalMedio>,
    pub mas_vendidos: Vec<ProductoVendido>,
    pub bajo_minimo: Vec<BajoMinimo>,
    pub total_bajo_minimo: i64,
    pub atrasados: Vec<ClienteAtrasado>,
    pub dias_alerta: i64,
}

/// Todo lo de la pantalla Hoy en una llamada.
pub async fn hoy(
    pool: &SqlitePool,
    zona: &dyn Zona,
    ahora: DateTime<Utc>,
) -> Resultado<ResumenHoy> {
    let dia = zona.dia(ahora);
    let (inicio, fin) = zona::limites(zona, dia, dia);
    let cfg = config::obtener(pool).await?;
    let mut conn = pool.acquire().await?;

    let horas = a_local(zona, repo::ventas_por_hora(&mut conn, &inicio, &fin).await?);
    let mut por_hora = vec![Celda::default(); 24];
    let mut total = Celda::default();
    let mut fiado = 0;
    for h in &horas {
        por_hora[h.local.hour() as usize].sumar(h.monto, h.boletas);
        total.sumar(h.monto, h.boletas);
        if h.medio == MedioPago::Fiado {
            fiado += h.monto;
        }
    }

    // El mismo día de la semana pasada, hasta la misma hora.
    let antes = dia - Duration::days(7);
    let inicio_antes = zona::a_texto(zona.inicio_dia(antes));
    let hasta_antes = zona::a_texto(ahora - Duration::days(7));
    let semana_anterior: i64 = repo::ventas_por_hora(&mut conn, &inicio_antes, &hasta_antes)
        .await?
        .iter()
        .map(|f| f.monto)
        .sum();

    let categorias = nombres_categorias(&mut conn).await?;
    let netos = netos(&mut conn, &inicio, &fin).await?;
    let vendidos = productos(&netos, &categorias);
    let ganancia = vendidos.iter().filter_map(|p| p.ganancia).sum();
    let ventas_sin_costo = vendidos.iter().map(|p| p.monto_sin_costo).sum();

    let pagos_fiado = repo::pagos_fiado(&mut conn, &inicio, &fin).await?;
    let bajo_minimo = repo::bajo_minimo(&mut conn, 10)
        .await?
        .into_iter()
        .map(|f| BajoMinimo {
            id: f.id,
            nombre: f.nombre,
            unidad: if f.unidad == "kg" {
                Unidad::Kg
            } else {
                Unidad::Unidad
            },
            stock_mil: f.stock_mil,
            stock_minimo_mil: f.stock_minimo_mil,
        })
        .collect();
    drop(conn);
    let total_bajo_minimo = crate::repositorios::inventario::contar_bajo_minimo(pool).await?;

    let dias_alerta = i64::from(cfg.fiado_dias_alerta);
    let limite_atraso = zona::a_texto(ahora - Duration::days(dias_alerta));
    let mut conn = pool.acquire().await?;
    let atrasados = repo::clientes_atrasados(&mut conn, &limite_atraso, 10)
        .await?
        .into_iter()
        .map(|f| ClienteAtrasado {
            dias: zona::de_texto(&f.desde).map_or(0, |d| (ahora - d).num_days()),
            id: f.id,
            nombre: f.nombre,
            deuda: f.deuda,
            desde: f.desde,
        })
        .collect();
    drop(conn);

    let caja = turnos::estado_caja(pool).await?;
    let efectivo_esperado = match caja.turno {
        Some(t) => Some(turnos::resumen(pool, t.id).await?.efectivo.esperado),
        None => None,
    };

    Ok(ResumenHoy {
        fecha: dia.format("%Y-%m-%d").to_string(),
        ventas: total.monto,
        boletas: total.boletas,
        ticket_promedio: promedio(total.monto, total.boletas),
        ganancia,
        ventas_sin_costo,
        ventas_semana_anterior: semana_anterior,
        variacion_pb: puntos_basicos(total.monto - semana_anterior, semana_anterior),
        efectivo_esperado,
        fiado_otorgado: fiado,
        pagos_fiado,
        por_hora,
        por_medio: por_medio(&horas),
        mas_vendidos: vendidos.into_iter().take(10).collect(),
        bajo_minimo,
        total_bajo_minimo,
        atrasados,
        dias_alerta,
    })
}

// --- Reporte de ventas ----------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VentasDia {
    /// "aaaa-mm-dd" local.
    pub dia: String,
    pub monto: i64,
    pub boletas: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VentasCategoria {
    pub categoria_id: Option<i64>,
    pub nombre: String,
    pub monto: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReporteVentas {
    pub desde: String,
    pub hasta: String,
    pub total: i64,
    pub boletas: i64,
    pub ticket_promedio: i64,
    /// Todos los días del rango, también los sin ventas.
    pub por_dia: Vec<VentasDia>,
    pub por_medio: Vec<TotalMedio>,
    /// Por categoría con el detalle de las líneas (no incluye redondeos ni descuentos de la boleta).
    pub por_categoria: Vec<VentasCategoria>,
    /// Lunes a domingo.
    pub por_dia_semana: Vec<Celda>,
    /// 7 × 24: día de la semana (lunes primero) por hora local.
    pub mapa: Vec<Vec<Celda>>,
}

pub async fn ventas(pool: &SqlitePool, zona: &dyn Zona, rango: Rango) -> Resultado<ReporteVentas> {
    let (inicio, fin) = rango.limites(zona);
    let mut conn = pool.acquire().await?;
    let horas = a_local(zona, repo::ventas_por_hora(&mut conn, &inicio, &fin).await?);

    let mut dias: BTreeMap<NaiveDate, Celda> =
        rango.dias().map(|d| (d, Celda::default())).collect();
    let mut semana = vec![Celda::default(); 7];
    let mut mapa = vec![vec![Celda::default(); 24]; 7];
    let mut total = Celda::default();
    for h in &horas {
        let d = h.local.date();
        let sem = d.weekday().num_days_from_monday() as usize;
        dias.entry(d).or_default().sumar(h.monto, h.boletas);
        semana[sem].sumar(h.monto, h.boletas);
        mapa[sem][h.local.hour() as usize].sumar(h.monto, h.boletas);
        total.sumar(h.monto, h.boletas);
    }

    let categorias = nombres_categorias(&mut conn).await?;
    let mut por_cat: BTreeMap<Option<i64>, i64> = BTreeMap::new();
    for n in netos(&mut conn, &inicio, &fin).await?.values() {
        *por_cat.entry(n.categoria_id).or_default() += n.monto;
    }
    let mut por_categoria: Vec<VentasCategoria> = por_cat
        .into_iter()
        .map(|(id, monto)| VentasCategoria {
            categoria_id: id,
            nombre: id
                .and_then(|c| categorias.get(&c).cloned())
                .unwrap_or_else(|| "Sin categoría".into()),
            monto,
        })
        .collect();
    por_categoria.sort_by_key(|c| std::cmp::Reverse(c.monto));

    Ok(ReporteVentas {
        desde: rango.desde.to_string(),
        hasta: rango.hasta.to_string(),
        total: total.monto,
        boletas: total.boletas,
        ticket_promedio: promedio(total.monto, total.boletas),
        por_dia: dias
            .into_iter()
            .map(|(d, c)| VentasDia {
                dia: d.to_string(),
                monto: c.monto,
                boletas: c.boletas,
            })
            .collect(),
        por_medio: por_medio(&horas),
        por_categoria,
        por_dia_semana: semana,
        mapa,
    })
}

// --- Reporte de productos -----------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SinVentas {
    pub producto_id: i64,
    pub nombre: String,
    pub unidad: Unidad,
    pub categoria: Option<String>,
    pub stock_mil: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReporteProductos {
    pub desde: String,
    pub hasta: String,
    /// De mayor a menor monto (la vista también ordena por cantidad).
    pub productos: Vec<ProductoVendido>,
    /// Productos activos sin ventas en el rango.
    pub sin_ventas: Vec<SinVentas>,
}

/// `categoria`: `None` todas, `Some(0)` sin categoría, `Some(id)` esa categoría.
pub async fn productos_vendidos(
    pool: &SqlitePool,
    zona: &dyn Zona,
    rango: Rango,
    categoria: Option<i64>,
) -> Resultado<ReporteProductos> {
    let (inicio, fin) = rango.limites(zona);
    let mut conn = pool.acquire().await?;
    let categorias = nombres_categorias(&mut conn).await?;
    let netos = netos(&mut conn, &inicio, &fin).await?;
    let entra = |c: Option<i64>| match categoria {
        None => true,
        Some(0) => c.is_none(),
        Some(id) => c == Some(id),
    };
    let lista = productos(&netos, &categorias)
        .into_iter()
        .filter(|p| entra(p.categoria_id))
        .collect();
    let sin_ventas = repo::productos_activos(&mut conn)
        .await?
        .into_iter()
        .filter(|p| p.activo && entra(p.categoria_id) && !netos.contains_key(&p.id))
        .map(|p| SinVentas {
            producto_id: p.id,
            categoria: p.categoria_id.and_then(|c| categorias.get(&c).cloned()),
            unidad: if p.unidad == "kg" {
                Unidad::Kg
            } else {
                Unidad::Unidad
            },
            nombre: p.nombre,
            stock_mil: p.stock_mil,
        })
        .collect();
    Ok(ReporteProductos {
        desde: rango.desde.to_string(),
        hasta: rango.hasta.to_string(),
        productos: lista,
        sin_ventas,
    })
}

// --- Reporte de ganancia ------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GananciaCategoria {
    pub categoria_id: Option<i64>,
    pub nombre: String,
    pub ventas: i64,
    pub costo: i64,
    pub ganancia: i64,
    pub margen_pb: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReporteGanancia {
    pub desde: String,
    pub hasta: String,
    /// Solo lo vendido con costo conocido.
    pub ventas: i64,
    pub costo: i64,
    pub ganancia: i64,
    pub margen_pb: Option<i64>,
    /// Parte de lo anterior que usa costos estimados (ventas antiguas con el costo actual).
    pub ventas_estimadas: i64,
    pub costo_estimado: i64,
    /// Lo vendido sin costo: no se inventa ganancia para esto.
    pub ventas_sin_costo: i64,
    /// Productos con costo, de mayor a menor ganancia.
    pub productos: Vec<ProductoVendido>,
    pub por_categoria: Vec<GananciaCategoria>,
    /// Productos vendidos que no tienen costo (o alguna de sus líneas no lo tenía).
    pub sin_costo: Vec<ProductoVendido>,
}

pub async fn ganancia(
    pool: &SqlitePool,
    zona: &dyn Zona,
    rango: Rango,
) -> Resultado<ReporteGanancia> {
    let (inicio, fin) = rango.limites(zona);
    let mut conn = pool.acquire().await?;
    let categorias = nombres_categorias(&mut conn).await?;
    let netos = netos(&mut conn, &inicio, &fin).await?;
    let todos = productos(&netos, &categorias);

    let mut cat: BTreeMap<Option<i64>, (i64, i64)> = BTreeMap::new();
    let (mut ventas, mut costo, mut v_est, mut c_est, mut sin) = (0, 0, 0, 0, 0);
    for n in netos.values() {
        ventas += n.monto_con_costo;
        costo += n.costo;
        v_est += n.monto_estimado;
        c_est += n.costo_estimado;
        sin += n.monto - n.monto_con_costo;
        if n.monto_con_costo != 0 || n.costo != 0 {
            let e = cat.entry(n.categoria_id).or_default();
            e.0 += n.monto_con_costo;
            e.1 += n.costo;
        }
    }
    let mut por_categoria: Vec<GananciaCategoria> = cat
        .into_iter()
        .map(|(id, (v, c))| GananciaCategoria {
            categoria_id: id,
            nombre: id
                .and_then(|x| categorias.get(&x).cloned())
                .unwrap_or_else(|| "Sin categoría".into()),
            ventas: v,
            costo: c,
            ganancia: v - c,
            margen_pb: puntos_basicos(v - c, v),
        })
        .collect();
    por_categoria.sort_by_key(|c| std::cmp::Reverse(c.ganancia));

    let sin_costo: Vec<ProductoVendido> = todos
        .iter()
        .filter(|p| p.monto_sin_costo != 0)
        .cloned()
        .collect();
    let mut con_costo: Vec<ProductoVendido> =
        todos.into_iter().filter(|p| p.ganancia.is_some()).collect();
    con_costo.sort_by(|a, b| {
        b.ganancia
            .cmp(&a.ganancia)
            .then_with(|| a.nombre.cmp(&b.nombre))
    });

    Ok(ReporteGanancia {
        desde: rango.desde.to_string(),
        hasta: rango.hasta.to_string(),
        ventas,
        costo,
        ganancia: ventas - costo,
        margen_pb: puntos_basicos(ventas - costo, ventas),
        ventas_estimadas: v_est,
        costo_estimado: c_est,
        ventas_sin_costo: sin,
        productos: con_costo,
        por_categoria,
        sin_costo,
    })
}

// --- Reporte de turnos --------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TurnoCerrado {
    pub id: i64,
    pub abierto_en: String,
    pub cerrado_en: String,
    pub monto_inicial: i64,
    pub esperado: i64,
    pub contado: i64,
    /// contado − esperado: positivo sobra, negativo falta.
    pub diferencia: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReporteTurnos {
    pub desde: String,
    pub hasta: String,
    pub turnos: Vec<TurnoCerrado>,
    pub sobrantes: i64,
    /// Lo que faltó, en positivo.
    pub faltantes: i64,
    pub cuadrados: i64,
}

pub async fn turnos(pool: &SqlitePool, zona: &dyn Zona, rango: Rango) -> Resultado<ReporteTurnos> {
    let (inicio, fin) = rango.limites(zona);
    let mut conn = pool.acquire().await?;
    let turnos: Vec<TurnoCerrado> = repo::turnos_cerrados(&mut conn, &inicio, &fin)
        .await?
        .into_iter()
        .map(|t| TurnoCerrado {
            id: t.id,
            abierto_en: t.abierto_en,
            cerrado_en: t.cerrado_en,
            monto_inicial: t.monto_inicial,
            esperado: t.efectivo_esperado,
            contado: t.efectivo_contado,
            diferencia: t.diferencia,
        })
        .collect();
    Ok(ReporteTurnos {
        desde: rango.desde.to_string(),
        hasta: rango.hasta.to_string(),
        sobrantes: turnos.iter().map(|t| t.diferencia.max(0)).sum(),
        faltantes: turnos.iter().map(|t| (-t.diferencia).max(0)).sum(),
        cuadrados: turnos.iter().filter(|t| t.diferencia == 0).count() as i64,
        turnos,
    })
}

// --- Reporte de fiados --------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TramoAntiguedad {
    /// "0-30", "31-60", "61-90" o "Más de 90".
    pub tramo: String,
    pub monto: i64,
    pub ventas: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeudorAlCierre {
    pub cliente_id: i64,
    pub nombre: String,
    pub deuda: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReporteFiados {
    pub desde: String,
    pub hasta: String,
    /// Lo que se debía al terminar el rango (o ahora, si el rango llega a hoy).
    pub deuda_al_cierre: i64,
    pub clientes_con_deuda: i64,
    /// Fiado en el rango (sin anuladas y descontando devoluciones) y pagos recibidos.
    pub otorgado: i64,
    pub pagado: i64,
    /// Antigüedad de la deuda al cierre, por la fecha de cada venta.
    pub antiguedad: Vec<TramoAntiguedad>,
    /// Los 20 que más debían al cierre.
    pub deudores: Vec<DeudorAlCierre>,
}

pub async fn fiados(
    pool: &SqlitePool,
    zona: &dyn Zona,
    rango: Rango,
    ahora: DateTime<Utc>,
) -> Resultado<ReporteFiados> {
    let (inicio, fin) = rango.limites(zona);
    // El cierre es el fin del rango, o ahora si todavía no llega.
    let cierre = zona::de_texto(&fin).map_or(ahora, |f| f.min(ahora));
    let cierre_texto = zona::a_texto(cierre);
    let mut conn = pool.acquire().await?;

    let otorgado = repo::ventas_por_hora(&mut conn, &inicio, &fin)
        .await?
        .iter()
        .filter(|f| f.medio_pago == "fiado")
        .map(|f| f.monto)
        .sum();
    let pagado = repo::pagos_fiado(&mut conn, &inicio, &fin).await?;

    let mut tramos = [(0i64, 0i64); 4];
    let mut por_cliente: HashMap<i64, (String, i64)> = HashMap::new();
    for v in repo::deuda_en(&mut conn, &cierre_texto).await? {
        if v.saldo <= 0 {
            continue;
        }
        let dias = zona::de_texto(&v.fecha_hora).map_or(0, |f| (cierre - f).num_days());
        let i = match dias {
            ..=30 => 0,
            31..=60 => 1,
            61..=90 => 2,
            _ => 3,
        };
        tramos[i].0 += v.saldo;
        tramos[i].1 += 1;
        por_cliente.entry(v.cliente_id).or_insert((v.nombre, 0)).1 += v.saldo;
    }
    let mut deudores: Vec<DeudorAlCierre> = por_cliente
        .into_iter()
        .map(|(id, (nombre, deuda))| DeudorAlCierre {
            cliente_id: id,
            nombre,
            deuda,
        })
        .collect();
    deudores.sort_by(|a, b| b.deuda.cmp(&a.deuda).then_with(|| a.nombre.cmp(&b.nombre)));
    let clientes_con_deuda = deudores.len() as i64;
    deudores.truncate(20);

    Ok(ReporteFiados {
        desde: rango.desde.to_string(),
        hasta: rango.hasta.to_string(),
        deuda_al_cierre: tramos.iter().map(|t| t.0).sum(),
        clientes_con_deuda,
        otorgado,
        pagado,
        antiguedad: ["0-30", "31-60", "61-90", "Más de 90"]
            .iter()
            .zip(tramos)
            .map(|(n, (monto, ventas))| TramoAntiguedad {
                tramo: (*n).into(),
                monto,
                ventas,
            })
            .collect(),
        deudores,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcentajes_en_puntos_basicos() {
        assert_eq!(puntos_basicos(1, 3), Some(3333));
        assert_eq!(puntos_basicos(2, 3), Some(6667));
        assert_eq!(puntos_basicos(-1, 3), Some(-3333));
        assert_eq!(puntos_basicos(5, 0), None);
        assert_eq!(promedio(10, 3), 3);
        assert_eq!(promedio(0, 0), 0);
    }
}
