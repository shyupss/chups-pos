//! Cada reporte por rango como [`Libro`]: las mismas cifras de la pantalla, en tablas para Excel
//! o PDF (con el nombre del negocio, el rango y la fecha de emisión).

use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::datos::{nombre_archivo, texto_rango, Columna, Libro, Tabla, Valor};
use crate::repositorios::productos::Unidad;
use crate::servicios::reportes::{self, ProductoVendido, Rango};
use crate::zona::Zona;
use crate::Resultado;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TipoReporte {
    Ventas,
    Productos,
    Ganancia,
    Turnos,
    Fiados,
}

const DIAS: [&str; 7] = [
    "Lunes",
    "Martes",
    "Miércoles",
    "Jueves",
    "Viernes",
    "Sábado",
    "Domingo",
];

fn resumen(filas: Vec<(&str, Valor)>) -> Tabla {
    let mut t = Tabla::new(
        "Resumen",
        vec![Columna::new("Concepto", 34), Columna::new("Valor", 18)],
    );
    for (c, v) in filas {
        t.fila(vec![Valor::texto(c), v]);
    }
    t
}

fn dia(texto: &str) -> Valor {
    NaiveDate::parse_from_str(texto, "%Y-%m-%d").map_or_else(|_| Valor::texto(texto), Valor::Dia)
}

fn cant(p: &ProductoVendido) -> Valor {
    Valor::Cantidad {
        mil: p.cantidad_mil,
        unidad: p.unidad,
    }
}

/// El libro de un reporte y el nombre base del archivo ("ventas-2026-11-01-al-2026-11-21").
pub async fn libro(
    pool: &SqlitePool,
    zona: &dyn Zona,
    tipo: TipoReporte,
    rango: Rango,
    categoria: Option<i64>,
    ahora: DateTime<Utc>,
) -> Resultado<(Libro, String)> {
    let negocio = {
        let mut conn = pool.acquire().await?;
        crate::repositorios::negocio::obtener(&mut conn)
            .await?
            .nombre
    };
    let (titulo, base, tablas) = match tipo {
        TipoReporte::Ventas => {
            let r = reportes::ventas(pool, zona, rango).await?;
            let mut por_dia = Tabla::new(
                "Por día",
                vec![
                    Columna::new("Día", 14),
                    Columna::new("Boletas", 10),
                    Columna::new("Ventas", 16),
                ],
            );
            for d in &r.por_dia {
                por_dia.fila(vec![
                    dia(&d.dia),
                    Valor::Entero(d.boletas),
                    Valor::Pesos(d.monto),
                ]);
            }
            let mut medios = Tabla::new(
                "Por medio de pago",
                vec![
                    Columna::new("Medio", 18),
                    Columna::new("Boletas", 10),
                    Columna::new("Ventas", 16),
                ],
            );
            for m in &r.por_medio {
                medios.fila(vec![
                    Valor::texto(m.medio.nombre()),
                    Valor::Entero(m.boletas),
                    Valor::Pesos(m.monto),
                ]);
            }
            let mut cats = Tabla::new(
                "Por categoría",
                vec![Columna::new("Categoría", 28), Columna::new("Ventas", 16)],
            );
            for c in &r.por_categoria {
                cats.fila(vec![Valor::texto(&c.nombre), Valor::Pesos(c.monto)]);
            }
            let mut semana = Tabla::new(
                "Por día de la semana",
                vec![
                    Columna::new("Día", 14),
                    Columna::new("Boletas", 10),
                    Columna::new("Ventas", 16),
                ],
            );
            for (i, c) in r.por_dia_semana.iter().enumerate() {
                semana.fila(vec![
                    Valor::texto(DIAS[i]),
                    Valor::Entero(c.boletas),
                    Valor::Pesos(c.monto),
                ]);
            }
            let mut columnas = vec![Columna::new("Día", 9)];
            columnas.extend((0..24).map(|h| Columna::new(&format!("{h}"), 4)));
            let mut mapa = Tabla::new("Boletas por día y hora", columnas);
            for (i, fila) in r.mapa.iter().enumerate() {
                let mut f = vec![Valor::texto(DIAS[i])];
                f.extend(fila.iter().map(|c| Valor::Entero(c.boletas)));
                mapa.fila(f);
            }
            (
                "Reporte de ventas",
                "ventas",
                vec![
                    resumen(vec![
                        ("Total vendido", Valor::Pesos(r.total)),
                        ("Boletas", Valor::Entero(r.boletas)),
                        ("Ticket promedio", Valor::Pesos(r.ticket_promedio)),
                    ]),
                    por_dia,
                    medios,
                    cats,
                    semana,
                    mapa,
                ],
            )
        }
        TipoReporte::Productos => {
            let r = reportes::productos_vendidos(pool, zona, rango, categoria).await?;
            let mut ranking = Tabla::new(
                "Más vendidos",
                vec![
                    Columna::new("Producto", 34),
                    Columna::new("Categoría", 16),
                    Columna::new("Cantidad", 12),
                    Columna::new("Unidad", 8),
                    Columna::new("Ventas", 14),
                ],
            );
            for p in &r.productos {
                ranking.fila(vec![
                    Valor::texto(&p.nombre),
                    Valor::opcional_texto(p.categoria.as_deref()),
                    cant(p),
                    Valor::texto(if p.unidad == Unidad::Kg {
                        "kg"
                    } else {
                        "unidad"
                    }),
                    Valor::Pesos(p.monto),
                ]);
            }
            let mut sin = Tabla::new(
                "Sin ventas",
                vec![
                    Columna::new("Producto", 34),
                    Columna::new("Categoría", 16),
                    Columna::new("Stock", 12),
                ],
            );
            for p in &r.sin_ventas {
                sin.fila(vec![
                    Valor::texto(&p.nombre),
                    Valor::opcional_texto(p.categoria.as_deref()),
                    Valor::Cantidad {
                        mil: p.stock_mil,
                        unidad: p.unidad,
                    },
                ]);
            }
            ("Reporte de productos", "productos", vec![ranking, sin])
        }
        TipoReporte::Ganancia => {
            let r = reportes::ganancia(pool, zona, rango).await?;
            let mut productos = Tabla::new(
                "Por producto",
                vec![
                    Columna::new("Producto", 30),
                    Columna::new("Cantidad", 11),
                    Columna::new("Ventas", 13),
                    Columna::new("Costo", 13),
                    Columna::new("Ganancia", 13),
                    Columna::new("Margen", 9),
                    Columna::new("Estimado", 9),
                ],
            );
            for p in &r.productos {
                productos.fila(vec![
                    Valor::texto(&p.nombre),
                    cant(p),
                    Valor::Pesos(p.monto_con_costo),
                    Valor::opcional_pesos(p.costo),
                    Valor::opcional_pesos(p.ganancia),
                    Valor::Porcentaje(p.margen_pb),
                    Valor::texto(if p.costo_estimado { "Sí" } else { "No" }),
                ]);
            }
            let mut cats = Tabla::new(
                "Por categoría",
                vec![
                    Columna::new("Categoría", 24),
                    Columna::new("Ventas", 13),
                    Columna::new("Costo", 13),
                    Columna::new("Ganancia", 13),
                    Columna::new("Margen", 9),
                ],
            );
            for c in &r.por_categoria {
                cats.fila(vec![
                    Valor::texto(&c.nombre),
                    Valor::Pesos(c.ventas),
                    Valor::Pesos(c.costo),
                    Valor::Pesos(c.ganancia),
                    Valor::Porcentaje(c.margen_pb),
                ]);
            }
            let mut sin = Tabla::new(
                "Sin costo",
                vec![
                    Columna::new("Producto", 34),
                    Columna::new("Vendido sin costo", 16),
                ],
            );
            for p in &r.sin_costo {
                sin.fila(vec![
                    Valor::texto(&p.nombre),
                    Valor::Pesos(p.monto_sin_costo),
                ]);
            }
            (
                "Reporte de ganancia",
                "ganancia",
                vec![
                    resumen(vec![
                        ("Ventas con costo", Valor::Pesos(r.ventas)),
                        ("Costo", Valor::Pesos(r.costo)),
                        ("Ganancia", Valor::Pesos(r.ganancia)),
                        ("Margen", Valor::Porcentaje(r.margen_pb)),
                        (
                            "Ventas con costo estimado",
                            Valor::Pesos(r.ventas_estimadas),
                        ),
                        ("Costo estimado", Valor::Pesos(r.costo_estimado)),
                        (
                            "Vendido sin costo (no cuenta)",
                            Valor::Pesos(r.ventas_sin_costo),
                        ),
                    ]),
                    productos,
                    cats,
                    sin,
                ],
            )
        }
        TipoReporte::Turnos => {
            let r = reportes::turnos(pool, zona, rango).await?;
            let mut lista = Tabla::new(
                "Turnos cerrados",
                vec![
                    Columna::new("Turno", 8),
                    Columna::new("Apertura", 17),
                    Columna::new("Cierre", 17),
                    Columna::new("Esperado", 13),
                    Columna::new("Contado", 13),
                    Columna::new("Diferencia", 13),
                ],
            );
            for t in &r.turnos {
                lista.fila(vec![
                    Valor::Entero(t.id),
                    Valor::Fecha(t.abierto_en.clone()),
                    Valor::Fecha(t.cerrado_en.clone()),
                    Valor::Pesos(t.esperado),
                    Valor::Pesos(t.contado),
                    Valor::Pesos(t.diferencia),
                ]);
            }
            (
                "Reporte de turnos",
                "turnos",
                vec![
                    resumen(vec![
                        ("Turnos cerrados", Valor::Entero(r.turnos.len() as i64)),
                        ("Cuadrados", Valor::Entero(r.cuadrados)),
                        ("Sobrantes", Valor::Pesos(r.sobrantes)),
                        ("Faltantes", Valor::Pesos(r.faltantes)),
                    ]),
                    lista,
                ],
            )
        }
        TipoReporte::Fiados => {
            let r = reportes::fiados(pool, zona, rango, ahora).await?;
            let mut tramos = Tabla::new(
                "Antigüedad de la deuda",
                vec![
                    Columna::new("Días", 12),
                    Columna::new("Ventas", 10),
                    Columna::new("Deuda", 14),
                ],
            );
            for t in &r.antiguedad {
                tramos.fila(vec![
                    Valor::texto(&t.tramo),
                    Valor::Entero(t.ventas),
                    Valor::Pesos(t.monto),
                ]);
            }
            let mut deudores = Tabla::new(
                "Los que más deben",
                vec![Columna::new("Cliente", 30), Columna::new("Deuda", 14)],
            );
            for d in &r.deudores {
                deudores.fila(vec![Valor::texto(&d.nombre), Valor::Pesos(d.deuda)]);
            }
            (
                "Reporte de fiados",
                "fiados",
                vec![
                    resumen(vec![
                        ("Deuda al cierre del rango", Valor::Pesos(r.deuda_al_cierre)),
                        ("Clientes con deuda", Valor::Entero(r.clientes_con_deuda)),
                        ("Fiado en el rango", Valor::Pesos(r.otorgado)),
                        ("Pagado en el rango", Valor::Pesos(r.pagado)),
                    ]),
                    tramos,
                    deudores,
                ],
            )
        }
    };
    Ok((
        Libro {
            titulo: titulo.into(),
            negocio,
            rango: texto_rango(rango.desde, rango.hasta),
            emitido: zona.a_local(ahora).format("%d-%m-%Y %H:%M").to_string(),
            tablas,
        },
        nombre_archivo(base, rango.desde, rango.hasta),
    ))
}
