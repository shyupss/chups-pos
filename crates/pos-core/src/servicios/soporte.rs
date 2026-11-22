//! Información para pedir soporte: lo necesario para entender un problema, sin datos de clientes,
//! sin el PIN y sin la licencia (solo su estado).

use crate::servicios::licencia::{EstadoLicencia, Modo};

/// Lo que junta el controlador; este módulo solo lo da formato.
#[derive(Debug, Clone)]
pub struct DatosSoporte {
    pub version_app: String,
    pub windows: String,
    pub version_esquema: i64,
    pub impresora: Option<String>,
    pub ruta_datos: String,
    pub tamano_base_bytes: u64,
    pub licencia: EstadoLicencia,
}

fn estado_licencia(l: &EstadoLicencia) -> String {
    match l.modo {
        Modo::Activa => match &l.licencia {
            Some(d) => format!("activa (n.º {}, {} equipo(s))", d.id, d.equipos),
            None => "activa".to_owned(),
        },
        Modo::Prueba => format!("en prueba, quedan {} día(s)", l.dias_restantes.unwrap_or(0)),
        Modo::Gracia => format!(
            "prueba terminada, quedan {} día(s) de gracia",
            l.dias_restantes.unwrap_or(0)
        ),
        Modo::Consulta => "modo consulta (sin licencia)".to_owned(),
    }
}

pub fn texto(d: &DatosSoporte) -> String {
    let mut t = String::new();
    let mut linea = |etiqueta: &str, valor: String| {
        t.push_str(&format!("{etiqueta}: {valor}\n"));
    };
    linea("CHUPS POS", d.version_app.clone());
    linea("Windows", d.windows.clone());
    linea(
        "Esquema de la base de datos",
        format!("v{}", d.version_esquema),
    );
    linea("Licencia", estado_licencia(&d.licencia));
    if let Some(e) = &d.licencia.error_licencia {
        linea("Problema con la licencia", e.clone());
    }
    if d.licencia.reloj_atrasado {
        linea("Reloj del equipo", "atrasado más de 48 horas".to_owned());
    }
    linea(
        "Impresora configurada",
        d.impresora.clone().unwrap_or_else(|| "ninguna".to_owned()),
    );
    linea("Carpeta de datos", d.ruta_datos.clone());
    linea(
        "Tamaño de la base",
        format!("{:.1} MB", d.tamano_base_bytes as f64 / 1_048_576.0),
    );
    t
}
