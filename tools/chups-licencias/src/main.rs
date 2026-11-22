//! chups-licencias: herramienta del vendedor de CHUPS POS (no va en el instalador).
//!
//!   chups-licencias generar-claves [--carpeta <dir>]
//!   chups-licencias emitir --negocio "Nombre" [--rut 12.345.678-5] [--solicitud CODIGO]
//!                          [--equipos 1] [--archivo licencia.chups-licencia]
//!   chups-licencias verificar <licencia o archivo> [--clave-publica HEX]
//!   chups-licencias listar
//!
//! La clave privada se lee de `CHUPS_CLAVE_PRIVADA` (64 dígitos hex), de la ruta en
//! `CHUPS_CLAVE_PRIVADA_ARCHIVO` o, si no, de `~/.chups-licencias/clave-privada.key`. Nunca se
//! guarda dentro de un repositorio git. El registro de lo emitido es un CSV en
//! `~/.chups-licencias/emitidas.csv` (o la ruta en `CHUPS_REGISTRO_LICENCIAS`).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pos_licencia::{
    clave_publica_de, firmar, generar_clave_privada, leer, Licencia, Solicitud, CLAVE_PUBLICA,
    EXTENSION,
};

const AYUDA: &str = "\
chups-licencias: emite licencias permanentes de CHUPS POS

Comandos:
  generar-claves [--carpeta <dir>]
      Crea la clave privada (clave-privada.key) y muestra la pública, que va en
      crates/pos-licencia/clave-publica.txt antes de compilar la app. Hazlo una sola vez:
      con otra clave, las licencias ya emitidas dejan de servir en las versiones nuevas.

  emitir --negocio \"Nombre\" [--rut RUT] [--solicitud CODIGO] [--equipos N] [--archivo RUTA]
      Firma una licencia. Con --solicitud (el código que muestra la app en Configuración >
      Licencia) queda atada a ese equipo; sin él, se ata al primer equipo donde se active.
      Queda anotada en el registro.

  verificar <licencia o archivo .chups-licencia> [--clave-publica HEX]
      Revisa la firma y muestra los datos.

  listar
      Muestra el registro de licencias emitidas.

Variables: CHUPS_CLAVE_PRIVADA (hex), CHUPS_CLAVE_PRIVADA_ARCHIVO, CHUPS_REGISTRO_LICENCIAS.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let resultado = match args.first().map(String::as_str) {
        Some("generar-claves") => generar_claves(&args[1..]),
        Some("emitir") => emitir(&args[1..]),
        Some("verificar") => verificar(&args[1..]),
        Some("listar") => listar(),
        Some("-h" | "--help" | "ayuda") | None => {
            println!("{AYUDA}");
            Ok(())
        }
        Some(otro) => Err(format!("Comando desconocido: {otro}\n\n{AYUDA}")),
    };
    match resultado {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Valor de `--nombre valor`.
fn opcion<'a>(args: &'a [String], nombre: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == nombre)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn carpeta_personal() -> Result<PathBuf, String> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|h| PathBuf::from(h).join(".chups-licencias"))
        .ok_or_else(|| "No se encontró la carpeta del usuario".to_owned())
}

/// La clave privada nunca se guarda dentro de un repositorio (ni de este ni de otro).
fn dentro_de_un_repo(ruta: &Path) -> bool {
    let absoluta = std::path::absolute(ruta).unwrap_or_else(|_| ruta.to_path_buf());
    absoluta.ancestors().any(|a| a.join(".git").exists())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn desde_hex(texto: &str) -> Result<[u8; 32], String> {
    let limpio: String = texto.chars().filter(|c| !c.is_whitespace()).collect();
    if limpio.len() != 64 {
        return Err("la clave debe tener 64 dígitos hexadecimales".into());
    }
    let mut salida = [0u8; 32];
    for (i, b) in salida.iter_mut().enumerate() {
        *b = u8::from_str_radix(&limpio[i * 2..i * 2 + 2], 16)
            .map_err(|_| "la clave debe ser hexadecimal".to_owned())?;
    }
    Ok(salida)
}

fn ruta_clave_privada() -> Result<PathBuf, String> {
    match std::env::var_os("CHUPS_CLAVE_PRIVADA_ARCHIVO") {
        Some(r) => Ok(PathBuf::from(r)),
        None => Ok(carpeta_personal()?.join("clave-privada.key")),
    }
}

fn clave_privada() -> Result<[u8; 32], String> {
    if let Ok(texto) = std::env::var("CHUPS_CLAVE_PRIVADA") {
        return desde_hex(&texto).map_err(|e| format!("CHUPS_CLAVE_PRIVADA: {e}"));
    }
    let ruta = ruta_clave_privada()?;
    let texto = fs::read_to_string(&ruta).map_err(|e| {
        format!(
            "No se pudo leer la clave privada en {} ({e}). Créala con `generar-claves` o usa CHUPS_CLAVE_PRIVADA.",
            ruta.display()
        )
    })?;
    desde_hex(&texto).map_err(|e| format!("{}: {e}", ruta.display()))
}

fn ruta_registro() -> Result<PathBuf, String> {
    match std::env::var_os("CHUPS_REGISTRO_LICENCIAS") {
        Some(r) => Ok(PathBuf::from(r)),
        None => Ok(carpeta_personal()?.join("emitidas.csv")),
    }
}

fn generar_claves(args: &[String]) -> Result<(), String> {
    let carpeta = match opcion(args, "--carpeta") {
        Some(c) => PathBuf::from(c),
        None => carpeta_personal()?,
    };
    if dentro_de_un_repo(&carpeta) {
        return Err(format!(
            "{} está dentro de un repositorio git: la clave privada no puede ir ahí. Elige otra carpeta con --carpeta.",
            carpeta.display()
        ));
    }
    let ruta = carpeta.join("clave-privada.key");
    if ruta.exists() {
        return Err(format!(
            "Ya existe {}. Si la reemplazas, las licencias emitidas dejan de servir en las versiones nuevas: bórrala a mano si de verdad quieres otra.",
            ruta.display()
        ));
    }
    fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
    let privada = generar_clave_privada()?;
    fs::write(&ruta, format!("{}\n", hex(&privada))).map_err(|e| e.to_string())?;
    println!("Clave privada guardada en {}", ruta.display());
    println!("Respáldala fuera del computador (por ejemplo, en un pendrive guardado): sin ella no puedes emitir licencias.\n");
    println!("Clave pública (va en crates/pos-licencia/clave-publica.txt, o en CHUPS_CLAVE_PUBLICA al compilar):");
    println!("{}", hex(&clave_publica_de(&privada)));
    Ok(())
}

fn emitir(args: &[String]) -> Result<(), String> {
    let negocio = opcion(args, "--negocio").ok_or("Falta --negocio \"Nombre del negocio\"")?;
    let rut = opcion(args, "--rut");
    let equipos: u8 = match opcion(args, "--equipos") {
        Some(n) => n.parse().map_err(|_| "--equipos debe ser un número")?,
        None => 1,
    };
    let solicitud = opcion(args, "--solicitud")
        .map(Solicitud::desde_texto)
        .transpose()
        .map_err(|e| e.to_string())?;
    let privada = clave_privada()?;
    let publica = clave_publica_de(&privada);

    let lic = Licencia::nueva(negocio, rut, equipos, solicitud.map(|s| s.huella))
        .map_err(|e| e.to_string())?;
    let texto = firmar(&lic, &privada).map_err(|e| e.to_string())?;
    // Antes de entregarla, se lee de vuelta con la clave pública.
    leer(&texto, &publica).map_err(|e| format!("La licencia emitida no se pudo verificar: {e}"))?;

    if publica != CLAVE_PUBLICA {
        eprintln!(
            "Aviso: esta clave privada no corresponde a la clave pública incrustada en esta compilación \
             ({}…). La app solo aceptará la licencia si se compiló con la clave pública {}.",
            &hex(&CLAVE_PUBLICA)[..8],
            hex(&publica)
        );
    }

    if let Some(ruta) = opcion(args, "--archivo") {
        let mut ruta = PathBuf::from(ruta);
        if ruta.extension().is_none() {
            ruta.set_extension(EXTENSION);
        }
        let contenido = format!(
            "# Licencia de CHUPS POS para {}\n# Cárgala en Configuración > Licencia > Cargar archivo.\n{texto}\n",
            lic.negocio
        );
        fs::write(&ruta, contenido).map_err(|e| e.to_string())?;
        println!("Archivo: {}", ruta.display());
    }

    anotar(&lic, solicitud.as_ref(), &texto)?;
    println!("Negocio: {}", lic.negocio);
    if let Some(r) = &lic.rut {
        println!("RUT: {r}");
    }
    println!(
        "Id: {}  Emitida: {}  Equipos: {}",
        lic.id, lic.emitida_en, lic.equipos
    );
    match (&lic.equipo, &solicitud) {
        (Some(h), Some(s)) => println!(
            "Atada al equipo {} (app {})",
            h.como_texto(),
            s.version_texto()
        ),
        _ => println!("Sin equipo: se atará al primer computador donde se active"),
    }
    println!("\n{texto}");
    Ok(())
}

/// Agrega una fila al registro CSV (lo crea con encabezado si no existe).
fn anotar(lic: &Licencia, solicitud: Option<&Solicitud>, texto: &str) -> Result<(), String> {
    let ruta = ruta_registro()?;
    if dentro_de_un_repo(&ruta) {
        return Err(format!(
            "El registro {} está dentro de un repositorio git: usa otra ruta (CHUPS_REGISTRO_LICENCIAS).",
            ruta.display()
        ));
    }
    if let Some(carpeta) = ruta.parent() {
        fs::create_dir_all(carpeta).map_err(|e| e.to_string())?;
    }
    let nuevo = !ruta.exists();
    let mut archivo = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ruta)
        .map_err(|e| format!("No se pudo abrir el registro {}: {e}", ruta.display()))?;
    let campo = |t: &str| format!("\"{}\"", t.replace('"', "\"\""));
    if nuevo {
        writeln!(
            archivo,
            "id,emitida_en,negocio,rut,equipos,equipo,version_app,licencia"
        )
        .map_err(|e| e.to_string())?;
    }
    writeln!(
        archivo,
        "{},{},{},{},{},{},{},{}",
        lic.id,
        lic.emitida_en,
        campo(&lic.negocio),
        campo(lic.rut.as_deref().unwrap_or("")),
        lic.equipos,
        lic.equipo.map(|h| h.como_texto()).unwrap_or_default(),
        solicitud.map(Solicitud::version_texto).unwrap_or_default(),
        texto
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn verificar(args: &[String]) -> Result<(), String> {
    let entrada = args
        .iter()
        .find(|a| !a.starts_with("--") && Some(a.as_str()) != opcion(args, "--clave-publica"))
        .ok_or("Falta la licencia o la ruta del archivo")?;
    let texto = if Path::new(entrada).is_file() {
        fs::read_to_string(entrada).map_err(|e| e.to_string())?
    } else {
        entrada.clone()
    };
    let publica = match opcion(args, "--clave-publica") {
        Some(h) => desde_hex(h)?,
        None => CLAVE_PUBLICA,
    };
    let lic = leer(&texto, &publica).map_err(|e| e.to_string())?;
    println!("Firma válida.");
    println!("Negocio: {}", lic.negocio);
    println!("RUT: {}", lic.rut.as_deref().unwrap_or("—"));
    println!(
        "Id: {}  Emitida: {}  Equipos: {}",
        lic.id, lic.emitida_en, lic.equipos
    );
    println!(
        "Equipo: {}",
        lic.equipo
            .map(|h| h.como_texto())
            .unwrap_or_else(|| "sin atar (se ata al primero donde se active)".into())
    );
    Ok(())
}

fn listar() -> Result<(), String> {
    let ruta = ruta_registro()?;
    match fs::read_to_string(&ruta) {
        Ok(t) => {
            println!("{}", ruta.display());
            print!("{t}");
            Ok(())
        }
        Err(_) => {
            println!("Todavía no hay licencias emitidas ({}).", ruta.display());
            Ok(())
        }
    }
}
