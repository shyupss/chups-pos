use std::path::PathBuf;
use std::sync::Mutex;

use sqlx::SqlitePool;

/// Estado compartido por todos los comandos. Se crea una vez al arrancar.
pub struct EstadoApp {
    pub pool: SqlitePool,
    /// Solo para leer (reportes y exportaciones), con mmap: el pool que vende no lo usa.
    pub lectura: SqlitePool,
    pub ruta_db: PathBuf,
    /// Milisegundos desde que arrancó el proceso hasta que la base quedó lista.
    pub ms_inicio: u64,
    /// Donde van los PDF si no se eligió otra carpeta: Documentos\CHUPS POS\Boletas.
    pub carpeta_pdf: PathBuf,
    /// Impresoras del sistema y la virtual de pruebas (escribe en la carpeta de datos).
    pub hardware: pos_hardware::Hardware,
    /// Intentos fallidos del PIN de administrador y su bloqueo (solo en memoria).
    pub pin: pos_core::servicios::seguridad::ControlPin,
    /// Respaldos si no se eligió otra carpeta: Respaldos junto a los datos de la app.
    pub carpeta_respaldos: PathBuf,
    /// El último respaldo automático que falló (o una restauración que no se pudo hacer).
    pub error_respaldo: Mutex<Option<String>>,
    /// Clave pública, huella de este equipo y archivo de la prueba, para la licencia.
    pub licencia: pos_core::servicios::licencia::Contexto,
    /// La actualización encontrada y si se pidió instalarla al cerrar el turno (solo en memoria).
    pub actualizacion: Mutex<crate::comandos::actualizaciones::MemoriaActualizacion>,
}
