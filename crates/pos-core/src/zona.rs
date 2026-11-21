//! Días y horas locales. La base guarda UTC; los reportes agrupan por el día y la hora del
//! computador. Los límites de un rango se calculan aquí, en UTC, para filtrar con los índices
//! por fecha, y cada hora UTC se traduce a su hora local para agrupar.
//!
//! Se agrupa por hora UTC y no por minuto porque todos los husos de Chile (y la gran mayoría)
//! están a horas enteras de UTC: una hora UTC cae entera en una hora local.

use chrono::{DateTime, Duration, FixedOffset, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};

/// Una zona horaria: la del computador en la app, una fija en las pruebas.
pub trait Zona: Send + Sync {
    /// Fecha y hora local de un instante.
    fn a_local(&self, utc: DateTime<Utc>) -> NaiveDateTime;
    /// Instantes UTC que corresponden a una fecha y hora local: ninguno si el cambio de hora se
    /// la salta, dos si se repite.
    fn desde_local(&self, local: NaiveDateTime) -> Vec<DateTime<Utc>>;

    /// Primer instante del día local. Si la medianoche no existe (en Chile el cambio de
    /// septiembre pasa de 23:59 a 01:00), la primera hora que sí.
    fn inicio_dia(&self, dia: NaiveDate) -> DateTime<Utc> {
        let medianoche = dia.and_hms_opt(0, 0, 0).expect("00:00 es una hora válida");
        (0..=3)
            .find_map(|h| {
                self.desde_local(medianoche + Duration::hours(h))
                    .into_iter()
                    .min()
            })
            .unwrap_or_else(|| Utc.from_utc_datetime(&medianoche))
    }

    /// Día local de un instante.
    fn dia(&self, utc: DateTime<Utc>) -> NaiveDate {
        self.a_local(utc).date()
    }
}

/// La zona horaria del computador (con sus cambios de horario).
pub struct ZonaLocal;

impl Zona for ZonaLocal {
    fn a_local(&self, utc: DateTime<Utc>) -> NaiveDateTime {
        utc.with_timezone(&Local).naive_local()
    }
    fn desde_local(&self, local: NaiveDateTime) -> Vec<DateTime<Utc>> {
        match Local.from_local_datetime(&local) {
            chrono::LocalResult::Single(t) => vec![t.with_timezone(&Utc)],
            chrono::LocalResult::Ambiguous(a, b) => {
                vec![a.with_timezone(&Utc), b.with_timezone(&Utc)]
            }
            chrono::LocalResult::None => vec![],
        }
    }
}

/// Zona con un único cambio de horario, para probar los reportes sin depender del computador:
/// antes de `cambio` rige `antes` y desde `cambio`, `despues`.
pub struct ZonaConCambio {
    pub cambio: DateTime<Utc>,
    pub antes: FixedOffset,
    pub despues: FixedOffset,
}

impl ZonaConCambio {
    /// Chile continental en 2026: el 5 de abril a las 00:00 (UTC−3) se vuelve a las 23:00 del 4
    /// (UTC−4), así que el 4 de abril tiene 25 horas.
    pub fn chile_abril_2026() -> Self {
        ZonaConCambio {
            cambio: Utc.with_ymd_and_hms(2026, 4, 5, 3, 0, 0).unwrap(),
            antes: FixedOffset::west_opt(3 * 3600).unwrap(),
            despues: FixedOffset::west_opt(4 * 3600).unwrap(),
        }
    }

    /// Chile continental en 2026: el 6 de septiembre a las 00:00 (UTC−4) se pasa a la 01:00
    /// (UTC−3), así que ese día no tiene medianoche y dura 23 horas.
    pub fn chile_septiembre_2026() -> Self {
        ZonaConCambio {
            cambio: Utc.with_ymd_and_hms(2026, 9, 6, 4, 0, 0).unwrap(),
            antes: FixedOffset::west_opt(4 * 3600).unwrap(),
            despues: FixedOffset::west_opt(3 * 3600).unwrap(),
        }
    }

    fn desfase(&self, utc: DateTime<Utc>) -> FixedOffset {
        if utc < self.cambio {
            self.antes
        } else {
            self.despues
        }
    }
}

impl Zona for ZonaConCambio {
    fn a_local(&self, utc: DateTime<Utc>) -> NaiveDateTime {
        utc.with_timezone(&self.desfase(utc)).naive_local()
    }
    fn desde_local(&self, local: NaiveDateTime) -> Vec<DateTime<Utc>> {
        let mut instantes: Vec<DateTime<Utc>> = [self.antes, self.despues]
            .into_iter()
            .filter_map(|d| d.from_local_datetime(&local).single())
            .map(|t| t.with_timezone(&Utc))
            .filter(|t| self.a_local(*t) == local)
            .collect();
        instantes.sort();
        instantes.dedup();
        instantes
    }
}

/// Instante como lo guarda la base: "2026-11-21T14:05:00.000Z" (se compara como texto).
pub fn a_texto(utc: DateTime<Utc>) -> String {
    utc.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Lee un instante guardado en la base.
pub fn de_texto(texto: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(texto)
        .ok()
        .map(|t| t.with_timezone(&Utc))
}

/// La hora UTC "2026-11-21T14" que entrega `substr(fecha_hora, 1, 13)`.
pub fn de_hora_utc(texto: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(&format!("{texto}:00:00"), "%Y-%m-%dT%H:%M:%S")
        .ok()
        .map(|t| Utc.from_utc_datetime(&t))
}

/// Rango de días locales, ambos incluidos, como límites UTC `[inicio, fin)`.
pub fn limites(zona: &dyn Zona, desde: NaiveDate, hasta: NaiveDate) -> (String, String) {
    (
        a_texto(zona.inicio_dia(desde)),
        a_texto(zona.inicio_dia(hasta + Duration::days(1))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(a: i32, m: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, dia).unwrap()
    }

    #[test]
    fn el_4_de_abril_dura_25_horas() {
        let z = ZonaConCambio::chile_abril_2026();
        let inicio = z.inicio_dia(d(2026, 4, 4));
        let fin = z.inicio_dia(d(2026, 4, 5));
        assert_eq!(a_texto(inicio), "2026-04-04T03:00:00.000Z");
        assert_eq!(a_texto(fin), "2026-04-05T04:00:00.000Z");
        assert_eq!((fin - inicio).num_hours(), 25);
        // Las 23:30 del 4 ocurren dos veces y las dos son del 4.
        let repetidas = z.desde_local(d(2026, 4, 4).and_hms_opt(23, 30, 0).unwrap());
        assert_eq!(repetidas.len(), 2);
        assert!(repetidas.iter().all(|t| z.dia(*t) == d(2026, 4, 4)));
    }

    #[test]
    fn el_6_de_septiembre_no_tiene_medianoche() {
        let z = ZonaConCambio::chile_septiembre_2026();
        let inicio = z.inicio_dia(d(2026, 9, 6));
        // Empieza a la 01:00 local (UTC−3) = 04:00 UTC, que es el mismo instante del cambio.
        assert_eq!(a_texto(inicio), "2026-09-06T04:00:00.000Z");
        assert_eq!(z.a_local(inicio).time().to_string(), "01:00:00");
        let fin = z.inicio_dia(d(2026, 9, 7));
        assert_eq!((fin - inicio).num_hours(), 23);
        assert_eq!(
            limites(&z, d(2026, 9, 5), d(2026, 9, 6)),
            (
                "2026-09-05T04:00:00.000Z".to_owned(),
                "2026-09-07T03:00:00.000Z".to_owned()
            )
        );
    }

    #[test]
    fn lee_las_fechas_de_la_base() {
        let t = de_texto("2026-11-21T14:05:09.123Z").unwrap();
        assert_eq!(a_texto(t), "2026-11-21T14:05:09.123Z");
        assert_eq!(
            de_hora_utc("2026-11-21T14").unwrap(),
            Utc.with_ymd_and_hms(2026, 11, 21, 14, 0, 0).unwrap()
        );
    }
}
