//! Importar productos y clientes desde una planilla. El flujo:
//!
//! 1. [`abrir`]: lee el archivo y propone qué columna es qué campo (por el nombre del
//!    encabezado); la vista deja cambiar la asignación a mano.
//! 2. `previsualizar_*`: separa las filas en nuevas, las que actualizan un registro (productos
//!    por código de barras; clientes por RUT o por nombre) y las que tienen errores, con el
//!    motivo. No guarda nada.
//! 3. `importar_*` (PIN "importar datos"): vuelve a leer el archivo, exige que no haya cambiado
//!    desde la vista previa y aplica todo en una transacción. Las filas con error se omiten.
//!
//! El stock nunca se escribe directo: entra como un movimiento 'ajuste' con motivo
//! "Importación". Las categorías que no existen se crean. Las deudas de los clientes no se
//! importan: solo nacen de ventas fiadas.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::boleta::formato::{cantidad, pesos};
use crate::datos::planilla::{self, Celda, Planilla};
use crate::error::validacion;
use crate::repositorios::clientes as repo_clientes;
use crate::repositorios::productos::{self as repo_productos, Campos, Movimiento, Unidad};
use crate::servicios::cantidades;
use crate::servicios::clientes::{self, DatosCliente};
use crate::servicios::productos::{self, DatosProducto};
use crate::servicios::seguridad::{self, Accion, ControlPin};
use crate::{rut, Error, Resultado};

/// El motivo de los movimientos de stock que deja una importación.
pub const MOTIVO: &str = "Importación";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CampoProducto {
    Codigo,
    Nombre,
    Categoria,
    Unidad,
    Precio,
    Costo,
    Stock,
    StockMinimo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CampoCliente {
    Nombre,
    Telefono,
    Rut,
    Direccion,
    Nota,
    Limite,
}

/// Qué se importa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tipo {
    Productos,
    Clientes,
}

/// Encabezado en minúsculas, sin tildes ni signos: "Código de barras" → "codigo de barras".
fn normalizar(texto: &str) -> String {
    let t: String = texto
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ñ' => 'n',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .collect();
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn tiene(h: &str, palabras: &[&str]) -> bool {
    palabras.iter().any(|p| {
        if p.contains(' ') || p.len() > 3 {
            h.contains(p)
        } else {
            // Las palabras cortas ("cod", "um", "min") solo como palabra completa.
            h.split(' ').any(|w| w == *p)
        }
    })
}

fn campo_producto(h: &str) -> Option<CampoProducto> {
    use CampoProducto::*;
    // El orden importa: "precio costo" es costo y "stock mínimo" es mínimo.
    Some(if tiene(h, &["minimo", "min", "critico"]) {
        StockMinimo
    } else if tiene(h, &["costo", "compra"]) {
        Costo
    } else if tiene(h, &["codigo", "barra", "ean", "sku", "cod"]) {
        Codigo
    } else if tiene(h, &["categoria", "familia", "rubro", "seccion"]) {
        Categoria
    } else if tiene(h, &["precio", "pvp", "valor", "venta"]) {
        Precio
    } else if tiene(h, &["unidad", "medida", "um", "formato de venta"]) {
        Unidad
    } else if tiene(h, &["stock", "existencia", "cantidad", "inventario"]) {
        Stock
    } else if tiene(
        h,
        &["nombre", "producto", "descripcion", "articulo", "detalle"],
    ) {
        Nombre
    } else {
        return None;
    })
}

fn campo_cliente(h: &str) -> Option<CampoCliente> {
    use CampoCliente::*;
    Some(if tiene(h, &["rut"]) {
        Rut
    } else if tiene(h, &["telefono", "fono", "celular", "movil", "whatsapp"]) {
        Telefono
    } else if tiene(h, &["direccion", "domicilio"]) {
        Direccion
    } else if tiene(h, &["nota", "observacion", "comentario"]) {
        Nota
    } else if tiene(h, &["limite", "credito", "cupo"]) {
        Limite
    } else if tiene(h, &["nombre", "cliente", "razon social"]) {
        Nombre
    } else {
        return None;
    })
}

/// Para cada columna, el campo que la lee (cada campo en una sola columna, la primera).
fn detectar<C: Copy + PartialEq>(
    encabezados: &[String],
    campo: fn(&str) -> Option<C>,
) -> Vec<Option<C>> {
    let mut usados = Vec::new();
    encabezados
        .iter()
        .map(|h| {
            let c = campo(&normalizar(h)).filter(|c| !usados.contains(c));
            if let Some(c) = c {
                usados.push(c);
            }
            c
        })
        .collect()
}

/// Lo que muestra la vista al elegir el archivo.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Apertura {
    pub ruta: String,
    pub encabezados: Vec<String>,
    /// Lo detectado por el nombre de cada encabezado (`productos` o `clientes`, según el tipo).
    pub productos: Vec<Option<CampoProducto>>,
    pub clientes: Vec<Option<CampoCliente>>,
    pub filas: usize,
    /// Las primeras filas como texto, para ver qué hay en cada columna.
    pub muestra: Vec<Vec<String>>,
}

/// Lee el archivo y propone la asignación de columnas. Es trabajo de disco: fuera de los hilos
/// async.
pub fn abrir(ruta: &Path) -> Resultado<Apertura> {
    let p = planilla::leer(ruta)?;
    Ok(Apertura {
        ruta: ruta.to_string_lossy().into_owned(),
        productos: detectar(&p.encabezados, campo_producto),
        clientes: detectar(&p.encabezados, campo_cliente),
        filas: p.filas.len(),
        muestra: p
            .filas
            .iter()
            .take(5)
            .map(|f| f.iter().map(Celda::texto).collect())
            .collect(),
        encabezados: p.encabezados,
    })
}

/// La columna de cada campo, validando que no haya dos columnas para el mismo.
fn columnas<C: Copy + Eq + std::hash::Hash>(mapeo: &[Option<C>]) -> Resultado<HashMap<C, usize>> {
    let mut m = HashMap::new();
    for (i, c) in mapeo.iter().enumerate() {
        if let Some(c) = c {
            if m.insert(*c, i).is_some() {
                return validacion("Hay dos columnas asignadas al mismo campo");
            }
        }
    }
    Ok(m)
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FilaError {
    /// Número de fila en la planilla.
    pub fila: usize,
    pub motivo: String,
}

// --- Productos ------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FilaProducto {
    pub fila: usize,
    pub codigo: Option<String>,
    pub nombre: String,
    pub categoria: Option<String>,
    pub unidad: Unidad,
    pub precio: i64,
    pub costo: Option<i64>,
    /// Stock que queda (si la planilla lo trae).
    pub stock_mil: Option<i64>,
    pub stock_minimo_mil: i64,
    /// Lo que cambia en un producto existente: "Precio $1.190 → $1.290".
    pub cambios: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VistaProductos {
    pub huella: String,
    pub nuevas: Vec<FilaProducto>,
    pub actualizan: Vec<FilaProducto>,
    pub errores: Vec<FilaError>,
    /// Filas de productos existentes que no cambian nada.
    pub sin_cambios: usize,
    pub categorias_nuevas: Vec<String>,
}

#[derive(Debug, Clone, FromRow)]
struct Existente {
    id: i64,
    codigo_barras: String,
    nombre: String,
    categoria_id: Option<i64>,
    unidad: Unidad,
    precio_venta: i64,
    costo: Option<i64>,
    stock_mil: i64,
    stock_minimo_mil: i64,
}

/// Lo que se va a hacer con una fila válida.
struct Plan {
    fila: FilaProducto,
    /// Producto que se actualiza (y su precio y stock antes).
    existente: Option<Existente>,
    /// Categoría por nombre (se crea si no existe); `None` sin categoría.
    categoria: Option<String>,
}

fn unidad(texto: &str) -> Result<Option<Unidad>, String> {
    let t = normalizar(texto);
    Ok(match t.as_str() {
        "" => None,
        "unidad" | "unidades" | "u" | "un" | "und" | "unid" | "c u" | "cu" | "uni" => {
            Some(Unidad::Unidad)
        }
        "kg" | "kgs" | "kilo" | "kilos" | "k" | "granel" | "kilogramo" | "kilogramos" | "peso" => {
            Some(Unidad::Kg)
        }
        _ => {
            return Err(format!(
                "La unidad «{texto}» no se entiende: usa unidad o kg"
            ))
        }
    })
}

async fn productos_con_codigo(
    conn: &mut SqliteConnection,
) -> Resultado<HashMap<String, Existente>> {
    let filas: Vec<Existente> = sqlx::query_as(
        "SELECT id, codigo_barras, nombre, categoria_id, unidad, precio_venta, costo, stock_mil,
                stock_minimo_mil
         FROM productos WHERE codigo_barras IS NOT NULL",
    )
    .fetch_all(conn)
    .await?;
    Ok(filas
        .into_iter()
        .map(|p| (p.codigo_barras.clone(), p))
        .collect())
}

/// Categorías existentes por nombre en minúsculas (la base las compara sin mayúsculas).
async fn categorias(conn: &mut SqliteConnection) -> Resultado<HashMap<String, (i64, String)>> {
    let filas: Vec<(i64, String)> = sqlx::query_as("SELECT id, nombre FROM categorias")
        .fetch_all(conn)
        .await?;
    Ok(filas
        .into_iter()
        .map(|(id, n)| (n.to_lowercase(), (id, n)))
        .collect())
}

/// Valida todas las filas sin escribir nada.
async fn planificar_productos(
    conn: &mut SqliteConnection,
    p: &Planilla,
    mapeo: &[Option<CampoProducto>],
) -> Resultado<(Vec<Plan>, Vec<FilaError>, usize, Vec<String>)> {
    use CampoProducto as C;
    let col = columnas(mapeo)?;
    if !col.contains_key(&C::Nombre) && !col.contains_key(&C::Codigo) {
        return validacion("Asigna al menos la columna del nombre o la del código de barras");
    }
    let existentes = productos_con_codigo(conn).await?;
    let cats = categorias(conn).await?;
    let nombres_cat: HashMap<i64, String> = cats.values().map(|(id, n)| (*id, n.clone())).collect();

    let mut planes = Vec::new();
    let mut errores = Vec::new();
    let mut sin_cambios = 0;
    let mut vistos: HashMap<String, usize> = HashMap::new();
    let mut nuevas_cat: Vec<String> = Vec::new();

    for (i, celdas) in p.filas.iter().enumerate() {
        if celdas.iter().all(Celda::vacia) {
            continue;
        }
        let fila = p.numero_fila(i);
        let celda = |c: C| col.get(&c).map(|&i| &celdas[i]);
        let texto = |c: C| celda(c).map(Celda::texto).filter(|t| !t.is_empty());
        let resultado = (|| -> Result<Plan, String> {
            let codigo = texto(C::Codigo);
            if let Some(c) = &codigo {
                if c.chars().any(char::is_whitespace) {
                    return Err(format!("El código «{c}» tiene espacios"));
                }
                if let Some(anterior) = vistos.insert(c.clone(), fila) {
                    return Err(format!("El código {c} ya está en la fila {anterior}"));
                }
            }
            let numero =
                |c: C, f: fn(&Celda) -> Result<Option<i64>, String>| celda(c).map_or(Ok(None), f);
            let precio = numero(C::Precio, planilla::pesos)?;
            let costo = numero(C::Costo, planilla::pesos)?;
            let stock = numero(C::Stock, planilla::cantidad)?;
            let minimo = numero(C::StockMinimo, planilla::cantidad)?;
            let unidad_dada = texto(C::Unidad).map_or(Ok(None), |u| unidad(&u))?;
            let categoria = texto(C::Categoria);
            let existente = codigo.as_ref().and_then(|c| existentes.get(c)).cloned();

            let fila_p =
                match &existente {
                    None => {
                        let nombre =
                            texto(C::Nombre).ok_or("Falta el nombre (es un producto nuevo)")?;
                        let precio = precio.ok_or("Falta el precio (es un producto nuevo)")?;
                        let unidad = unidad_dada.unwrap_or_default();
                        FilaProducto {
                            fila,
                            codigo: codigo.clone(),
                            nombre,
                            categoria: categoria.clone(),
                            unidad,
                            precio,
                            costo,
                            stock_mil: stock,
                            stock_minimo_mil: minimo.unwrap_or(0),
                            cambios: vec![],
                        }
                    }
                    Some(e) => {
                        if unidad_dada.is_some_and(|u| u != e.unidad) {
                            return Err(format!(
                            "{} se vende por {}: la unidad de un producto existente no se cambia",
                            e.nombre,
                            if e.unidad == Unidad::Kg { "kg" } else { "unidad" }
                        ));
                        }
                        let nombre = texto(C::Nombre).unwrap_or_else(|| e.nombre.clone());
                        let cat_actual = e.categoria_id.and_then(|c| nombres_cat.get(&c).cloned());
                        let categoria = categoria.clone().or(cat_actual.clone());
                        let precio = precio.unwrap_or(e.precio_venta);
                        let costo = costo.or(e.costo);
                        let minimo = minimo.unwrap_or(e.stock_minimo_mil);
                        let mut cambios = Vec::new();
                        if nombre != e.nombre {
                            cambios.push(format!("Nombre «{}» → «{nombre}»", e.nombre));
                        }
                        if categoria.as_deref().map(str::to_lowercase)
                            != cat_actual.as_deref().map(str::to_lowercase)
                        {
                            cambios.push(format!(
                                "Categoría {} → {}",
                                cat_actual.as_deref().unwrap_or("ninguna"),
                                categoria.as_deref().unwrap_or("ninguna")
                            ));
                        }
                        if precio != e.precio_venta {
                            cambios.push(format!(
                                "Precio {} → {}",
                                pesos(e.precio_venta),
                                pesos(precio)
                            ));
                        }
                        if costo != e.costo {
                            cambios.push(format!(
                                "Costo {} → {}",
                                e.costo.map_or("sin costo".into(), pesos),
                                costo.map_or("sin costo".into(), pesos)
                            ));
                        }
                        if let Some(s) = stock.filter(|s| *s != e.stock_mil) {
                            cambios.push(format!(
                                "Stock {} → {}",
                                cantidad(e.stock_mil, e.unidad),
                                cantidad(s, e.unidad)
                            ));
                        }
                        if minimo != e.stock_minimo_mil {
                            cambios.push(format!(
                                "Mínimo {} → {}",
                                cantidad(e.stock_minimo_mil, e.unidad),
                                cantidad(minimo, e.unidad)
                            ));
                        }
                        FilaProducto {
                            fila,
                            codigo: codigo.clone(),
                            nombre,
                            categoria,
                            unidad: e.unidad,
                            precio,
                            costo,
                            stock_mil: stock,
                            stock_minimo_mil: minimo,
                            cambios,
                        }
                    }
                };
            // Las mismas reglas que el formulario de producto.
            productos::validar(&DatosProducto {
                codigo_barras: fila_p.codigo.clone(),
                nombre: fila_p.nombre.clone(),
                categoria_id: None,
                unidad: fila_p.unidad,
                precio_venta: fila_p.precio,
                costo: fila_p.costo,
                stock_minimo_mil: fila_p.stock_minimo_mil,
                stock_inicial_mil: 0,
            })
            .map_err(|e| e.to_string())?;
            if let Some(s) = fila_p.stock_mil {
                cantidades::validar_stock(fila_p.unidad, s, "stock").map_err(|e| e.to_string())?;
            }
            if fila_p
                .categoria
                .as_ref()
                .is_some_and(|c| c.chars().count() > 60)
            {
                return Err("El nombre de la categoría no puede pasar de 60 caracteres".into());
            }
            Ok(Plan {
                categoria: fila_p.categoria.clone(),
                fila: fila_p,
                existente,
            })
        })();
        match resultado {
            Ok(plan) => {
                if let Some(c) = &plan.categoria {
                    let k = c.to_lowercase();
                    if !cats.contains_key(&k) && !nuevas_cat.iter().any(|n| n.to_lowercase() == k) {
                        nuevas_cat.push(c.clone());
                    }
                }
                if plan.existente.is_some() && plan.fila.cambios.is_empty() {
                    sin_cambios += 1;
                } else {
                    planes.push(plan);
                }
            }
            Err(motivo) => errores.push(FilaError { fila, motivo }),
        }
    }
    Ok((planes, errores, sin_cambios, nuevas_cat))
}

fn leer(ruta: &str) -> Resultado<Planilla> {
    planilla::leer(Path::new(ruta))
}

pub async fn previsualizar_productos(
    pool: &SqlitePool,
    ruta: &str,
    mapeo: &[Option<CampoProducto>],
) -> Resultado<VistaProductos> {
    let p = leer(ruta)?;
    let mut conn = pool.acquire().await?;
    let (planes, errores, sin_cambios, categorias_nuevas) =
        planificar_productos(&mut conn, &p, mapeo).await?;
    let (actualizan, nuevas): (Vec<Plan>, Vec<Plan>) =
        planes.into_iter().partition(|p| p.existente.is_some());
    Ok(VistaProductos {
        huella: p.huella,
        nuevas: nuevas.into_iter().map(|p| p.fila).collect(),
        actualizan: actualizan.into_iter().map(|p| p.fila).collect(),
        errores,
        sin_cambios,
        categorias_nuevas,
    })
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResultadoImportacion {
    pub creados: usize,
    pub actualizados: usize,
    /// Filas con error que no se importaron.
    pub omitidos: usize,
    pub categorias_creadas: usize,
}

fn misma_huella(p: &Planilla, huella: &str) -> Resultado<()> {
    if p.huella != huella {
        return validacion(
            "El archivo cambió desde la vista previa: vuelve a revisarlo antes de importar",
        );
    }
    Ok(())
}

/// Importa los productos válidos en una transacción (PIN "importar datos").
pub async fn importar_productos(
    pool: &SqlitePool,
    control: &ControlPin,
    ruta: &str,
    mapeo: &[Option<CampoProducto>],
    huella: &str,
    pin: Option<&str>,
) -> Resultado<ResultadoImportacion> {
    seguridad::exigir(pool, control, Accion::ImportarDatos, pin).await?;
    let p = leer(ruta)?;
    misma_huella(&p, huella)?;
    let mut tx = pool.begin().await?;
    let (planes, errores, _, nuevas_cat) = planificar_productos(&mut tx, &p, mapeo).await?;

    let mut cats: HashMap<String, i64> = categorias(&mut tx)
        .await?
        .into_iter()
        .map(|(k, (id, _))| (k, id))
        .collect();
    for nombre in &nuevas_cat {
        let id: i64 = sqlx::query_scalar("INSERT INTO categorias (nombre) VALUES (?) RETURNING id")
            .bind(nombre)
            .fetch_one(&mut *tx)
            .await?;
        cats.insert(nombre.to_lowercase(), id);
    }

    let (mut creados, mut actualizados) = (0, 0);
    for plan in &planes {
        let f = &plan.fila;
        let campos = Campos {
            codigo_barras: f.codigo.as_deref(),
            nombre: f.nombre.trim(),
            categoria_id: plan
                .categoria
                .as_ref()
                .and_then(|c| cats.get(&c.to_lowercase()).copied()),
            unidad: f.unidad,
            precio_venta: f.precio,
            costo: f.costo,
            stock_minimo_mil: f.stock_minimo_mil,
        };
        let (id, stock_antes) = match &plan.existente {
            Some(e) => {
                repo_productos::actualizar(&mut tx, e.id, &campos).await?;
                if e.precio_venta != f.precio {
                    repo_productos::registrar_cambio_precio(
                        &mut tx,
                        e.id,
                        e.precio_venta,
                        f.precio,
                    )
                    .await?;
                }
                actualizados += 1;
                (e.id, e.stock_mil)
            }
            None => {
                let id = repo_productos::insertar(&mut tx, &campos)
                    .await
                    .map_err(|e| match e {
                        Error::Conflicto(_) => Error::Validacion(format!(
                            "Fila {}: el código {} ya existe",
                            f.fila,
                            f.codigo.as_deref().unwrap_or("")
                        )),
                        otro => otro,
                    })?;
                creados += 1;
                (id, 0)
            }
        };
        if let Some(s) = f.stock_mil {
            if s != stock_antes {
                repo_productos::registrar_movimiento(
                    &mut tx,
                    &Movimiento {
                        producto_id: id,
                        delta_mil: s - stock_antes,
                        tipo: "ajuste",
                        venta_id: None,
                        compra_id: None,
                        nota: None,
                        motivo: Some(MOTIVO),
                    },
                )
                .await?;
            }
        }
    }
    tx.commit().await?;
    Ok(ResultadoImportacion {
        creados,
        actualizados,
        omitidos: errores.len(),
        categorias_creadas: nuevas_cat.len(),
    })
}

// --- Clientes -------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FilaCliente {
    pub fila: usize,
    pub nombre: String,
    pub telefono: Option<String>,
    pub rut: Option<String>,
    pub direccion: Option<String>,
    pub nota: Option<String>,
    pub limite_credito: Option<i64>,
    pub cambios: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VistaClientes {
    pub huella: String,
    pub nuevas: Vec<FilaCliente>,
    pub actualizan: Vec<FilaCliente>,
    pub errores: Vec<FilaError>,
    pub sin_cambios: usize,
}

#[derive(Debug, Clone, FromRow)]
struct ClienteExistente {
    id: i64,
    nombre: String,
    telefono: Option<String>,
    rut: Option<String>,
    direccion: Option<String>,
    nota: Option<String>,
    limite_credito: Option<i64>,
}

struct PlanCliente {
    fila: FilaCliente,
    existente: Option<i64>,
}

async fn planificar_clientes(
    conn: &mut SqliteConnection,
    p: &Planilla,
    mapeo: &[Option<CampoCliente>],
) -> Resultado<(Vec<PlanCliente>, Vec<FilaError>, usize)> {
    use CampoCliente as C;
    let col = columnas(mapeo)?;
    if !col.contains_key(&C::Nombre) {
        return validacion("Asigna la columna del nombre del cliente");
    }
    let existentes: Vec<ClienteExistente> = sqlx::query_as(
        "SELECT id, nombre, telefono, rut, direccion, nota, limite_credito FROM clientes",
    )
    .fetch_all(conn)
    .await?;
    let por_rut: HashMap<String, &ClienteExistente> = existentes
        .iter()
        .filter_map(|c| c.rut.clone().map(|r| (r, c)))
        .collect();
    let por_nombre: HashMap<String, &ClienteExistente> = existentes
        .iter()
        .map(|c| (c.nombre.to_lowercase(), c))
        .collect();

    let mut planes = Vec::new();
    let mut errores = Vec::new();
    let mut sin_cambios = 0;
    let mut vistos: HashSet<i64> = HashSet::new();
    for (i, celdas) in p.filas.iter().enumerate() {
        if celdas.iter().all(Celda::vacia) {
            continue;
        }
        let fila = p.numero_fila(i);
        let celda = |c: C| col.get(&c).map(|&i| &celdas[i]);
        let texto = |c: C| celda(c).map(Celda::texto).filter(|t| !t.is_empty());
        let resultado = (|| -> Result<Option<PlanCliente>, String> {
            let rut_dado = match texto(C::Rut) {
                Some(r) => Some(rut::normalizar(&r).map_err(|e| e.to_string())?),
                None => None,
            };
            let nombre = texto(C::Nombre);
            let limite = celda(C::Limite).map_or(Ok(None), planilla::pesos)?;
            let existente = rut_dado
                .as_ref()
                .and_then(|r| por_rut.get(r))
                .or_else(|| {
                    nombre
                        .as_ref()
                        .and_then(|n| por_nombre.get(&n.to_lowercase()))
                })
                .copied();
            let mut f = FilaCliente {
                fila,
                nombre: nombre.clone().unwrap_or_default(),
                telefono: texto(C::Telefono),
                rut: rut_dado,
                direccion: texto(C::Direccion),
                nota: texto(C::Nota),
                limite_credito: limite,
                cambios: vec![],
            };
            if let Some(e) = existente {
                if !vistos.insert(e.id) {
                    return Err(format!(
                        "{} aparece más de una vez en la planilla",
                        e.nombre
                    ));
                }
                // Lo que la planilla no trae, se mantiene.
                let mantener = |dado: &mut Option<String>,
                                actual: &Option<String>,
                                que: &str,
                                cambios: &mut Vec<String>| {
                    if dado.is_none() {
                        dado.clone_from(actual);
                    } else if dado != actual {
                        cambios.push(format!(
                            "{que} {} → {}",
                            actual.as_deref().unwrap_or("—"),
                            dado.as_deref().unwrap_or("—")
                        ));
                    }
                };
                let mut cambios = Vec::new();
                if f.nombre.is_empty() {
                    f.nombre.clone_from(&e.nombre);
                } else if f.nombre != e.nombre {
                    cambios.push(format!("Nombre «{}» → «{}»", e.nombre, f.nombre));
                }
                mantener(&mut f.telefono, &e.telefono, "Teléfono", &mut cambios);
                mantener(&mut f.rut, &e.rut, "RUT", &mut cambios);
                mantener(&mut f.direccion, &e.direccion, "Dirección", &mut cambios);
                mantener(&mut f.nota, &e.nota, "Nota", &mut cambios);
                if f.limite_credito.is_none() {
                    f.limite_credito = e.limite_credito;
                } else if f.limite_credito != e.limite_credito {
                    cambios.push(format!(
                        "Límite {} → {}",
                        e.limite_credito.map_or("sin límite".into(), pesos),
                        f.limite_credito.map_or("sin límite".into(), pesos)
                    ));
                }
                f.cambios = cambios;
            }
            clientes::validar(&DatosCliente {
                nombre: f.nombre.clone(),
                telefono: f.telefono.clone(),
                rut: f.rut.clone(),
                direccion: f.direccion.clone(),
                nota: f.nota.clone(),
                limite_credito: f.limite_credito,
            })
            .map_err(|e| e.to_string())?;
            if existente.is_some() && f.cambios.is_empty() {
                return Ok(None);
            }
            Ok(Some(PlanCliente {
                fila: f,
                existente: existente.map(|e| e.id),
            }))
        })();
        match resultado {
            Ok(Some(plan)) => planes.push(plan),
            Ok(None) => sin_cambios += 1,
            Err(motivo) => errores.push(FilaError { fila, motivo }),
        }
    }
    Ok((planes, errores, sin_cambios))
}

pub async fn previsualizar_clientes(
    pool: &SqlitePool,
    ruta: &str,
    mapeo: &[Option<CampoCliente>],
) -> Resultado<VistaClientes> {
    let p = leer(ruta)?;
    let mut conn = pool.acquire().await?;
    let (planes, errores, sin_cambios) = planificar_clientes(&mut conn, &p, mapeo).await?;
    let (actualizan, nuevas): (Vec<PlanCliente>, Vec<PlanCliente>) =
        planes.into_iter().partition(|p| p.existente.is_some());
    Ok(VistaClientes {
        huella: p.huella,
        nuevas: nuevas.into_iter().map(|p| p.fila).collect(),
        actualizan: actualizan.into_iter().map(|p| p.fila).collect(),
        errores,
        sin_cambios,
    })
}

/// Importa los clientes válidos en una transacción (PIN "importar datos"). Sin deudas.
pub async fn importar_clientes(
    pool: &SqlitePool,
    control: &ControlPin,
    ruta: &str,
    mapeo: &[Option<CampoCliente>],
    huella: &str,
    pin: Option<&str>,
) -> Resultado<ResultadoImportacion> {
    seguridad::exigir(pool, control, Accion::ImportarDatos, pin).await?;
    let p = leer(ruta)?;
    misma_huella(&p, huella)?;
    let mut tx = pool.begin().await?;
    let (planes, errores, _) = planificar_clientes(&mut tx, &p, mapeo).await?;
    let (mut creados, mut actualizados) = (0, 0);
    for plan in &planes {
        let f = &plan.fila;
        let l = clientes::validar(&DatosCliente {
            nombre: f.nombre.clone(),
            telefono: f.telefono.clone(),
            rut: f.rut.clone(),
            direccion: f.direccion.clone(),
            nota: f.nota.clone(),
            limite_credito: f.limite_credito,
        })?;
        match plan.existente {
            Some(id) => {
                repo_clientes::actualizar(&mut tx, id, &l.campos()).await?;
                actualizados += 1;
            }
            None => {
                repo_clientes::insertar(&mut tx, &l.campos()).await?;
                creados += 1;
            }
        }
    }
    tx.commit().await?;
    Ok(ResultadoImportacion {
        creados,
        actualizados,
        omitidos: errores.len(),
        categorias_creadas: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detecta_columnas_por_su_nombre() {
        let h: Vec<String> = [
            "Código de barras",
            "Descripción",
            "Familia",
            "U. de medida",
            "Precio de venta",
            "Precio costo",
            "Stock actual",
            "Stock mínimo",
            "Proveedor",
        ]
        .map(String::from)
        .to_vec();
        use CampoProducto::*;
        assert_eq!(
            detectar(&h, campo_producto),
            [
                Some(Codigo),
                Some(Nombre),
                Some(Categoria),
                Some(Unidad),
                Some(Precio),
                Some(Costo),
                Some(Stock),
                Some(StockMinimo),
                None
            ]
        );
        let h: Vec<String> = [
            "Nombre",
            "RUT",
            "Celular",
            "Dirección",
            "Cupo",
            "Observaciones",
        ]
        .map(String::from)
        .to_vec();
        use CampoCliente as K;
        assert_eq!(
            detectar(&h, campo_cliente),
            [
                Some(K::Nombre),
                Some(K::Rut),
                Some(K::Telefono),
                Some(K::Direccion),
                Some(K::Limite),
                Some(K::Nota)
            ]
        );
    }
}
