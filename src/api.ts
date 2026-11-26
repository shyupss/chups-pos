// Única puerta de la Vista hacia el Controlador (comandos Tauri en Rust).
// Cada comando tiene aquí su función tipada; las pantallas no llaman a `invoke` directo.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface EstadoSistema {
  version: string;
  rutaBaseDatos: string;
  versionEsquema: number;
  totalProductos: number;
  msInicio: number;
}

/** En productos por kg, precioVenta es el precio por kilo. */
export type Unidad = "unidad" | "kg";

// Cantidades y stocks en milésimas (enteros): 1 unidad = 1000, 0,532 kg = 532.
export interface Producto {
  id: number;
  codigoBarras: string | null;
  nombre: string;
  categoriaId: number | null;
  unidad: Unidad;
  precioVenta: number;
  costo: number | null;
  stockMil: number;
  stockMinimoMil: number;
  activo: boolean;
}

export interface DatosProducto {
  codigoBarras: string | null;
  nombre: string;
  categoriaId: number | null;
  unidad: Unidad;
  precioVenta: number;
  costo: number | null;
  stockMinimoMil: number;
  /** Solo se usa al crear. */
  stockInicialMil: number;
}

export interface Categoria {
  id: number;
  nombre: string;
  /** Productos activos de la categoría. */
  productos: number;
}

/** Filtro de la búsqueda de productos: todas, solo las sin categoría o una categoría. */
export type FiltroCategoria = "todas" | "sin" | number;

export type MedioPago = "efectivo" | "debito" | "credito" | "transferencia" | "fiado";

export interface ItemPedido {
  productoId: number;
  cantidadMil: number;
}

export interface NuevaVenta {
  items: ItemPedido[];
  medioPago: MedioPago;
  montoRecibido: number | null;
  descuento: number;
  clienteId: number | null;
}

export interface LineaVendida {
  productoId: number;
  nombre: string;
  precioUnitario: number;
  cantidadMil: number;
  subtotal: number;
  stockResultanteMil: number;
}

export interface VentaConfirmada {
  id: number;
  folio: number;
  fechaHora: string;
  subtotal: number;
  descuento: number;
  /** Ajuste del efectivo a la decena (puede ser negativo). total = subtotal - descuento + redondeo. */
  redondeo: number;
  total: number;
  medioPago: MedioPago;
  montoRecibido: number | null;
  vuelto: number | null;
  lineas: LineaVendida[];
  /** Fiado: el cliente y su deuda total después de esta venta. */
  cliente: string | null;
  deudaCliente: number | null;
}

export type TablaCaracteres = "cp858" | "cp850" | "wpc1252";
export type FormatoPdf = "ticket" | "carta";
export type FormatoBalanza = "peso" | "precio";

/** Tamaño de letra y botones de toda la interfaz: Rust lo aplica como zoom (100 %, 110 % y 125 %). */
export type EscalaInterfaz = "normal" | "grande" | "masgrande";

/** Configuración tipada de la tabla config (la valida el servicio de Rust). */
export interface Config {
  redondeoEfectivo: boolean;
  boletaLeyenda: boolean;
  boletaCodigo: boolean;
  impresora: string | null;
  anchoPapelMm: 58 | 80;
  tablaCaracteres: TablaCaracteres;
  imprimirAlCobrar: boolean;
  copias: 1 | 2;
  pdfFormato: FormatoPdf;
  pdfCarpeta: string | null;
  cajonActivo: boolean;
  cajonPin: 2 | 5;
  cajonPulsoMs: number;
  cajonSoloEfectivo: boolean;
  balanzaActiva: boolean;
  balanzaPrefijos: string;
  balanzaFormato: FormatoBalanza;
  balanzaLargoCodigo: number;
  /** Días desde la fiada pendiente más antigua a partir de los que la deuda se resalta. */
  fiadoDiasAlerta: number;
  /** null = Respaldos junto a los datos de la app. */
  respaldosCarpeta: string | null;
  respaldosConservar: number;
  escalaInterfaz: EscalaInterfaz;
}

/** Una venta en espera en la lista. El total usa los precios actuales de los productos activos. */
export interface ResumenEspera {
  id: number;
  nombre: string | null;
  creadaEn: string;
  lineas: number;
  totalEstimado: number;
}

export interface VentaRecuperada {
  lineas: { producto: Producto; cantidadMil: number }[];
  /** Líneas que no volvieron porque el producto se desactivó. */
  saltados: number;
  /** Si se intercambió, el id con que quedó en espera el carrito anterior. */
  intercambioId: number | null;
}

/** Estado que informa el driver de la impresora. */
export type EstadoImpresora =
  | { tipo: "lista" }
  | { tipo: "desconectada" }
  | { tipo: "conError"; detalle: string }
  | { tipo: "desconocido" };

export interface Impresora {
  nombre: string;
  predeterminada: boolean;
  estado: EstadoImpresora;
  /** "Archivo (pruebas)": escribe los bytes en un .bin. */
  esVirtual: boolean;
}

/** La impresora elegida en Configuración y su estado (null si no hay ninguna elegida). */
export interface EstadoConfigurada {
  impresora: string | null;
  estado: EstadoImpresora | null;
}

export interface Negocio {
  nombre: string;
  rut: string | null;
  direccion: string | null;
  telefono: string | null;
  mensajePie: string;
  /** PNG de 1 bit en base64 (para la vista previa). */
  logoPng: string | null;
}

export type DatosNegocio = Omit<Negocio, "logoPng">;

/** Acciones que pueden pedir el PIN de administrador (sin PIN creado, ninguna lo pide). */
export interface AccionesProtegidas {
  anularVenta: boolean;
  devolucion: boolean;
  retiroEfectivo: boolean;
  cerrarCaja: boolean;
  cambiarConfiguracion: boolean;
  /** Ajustes por conteo, mermas y anular compras. */
  ajustarInventario: boolean;
  /** Vender fiado por sobre el límite de crédito del cliente. */
  fiarSobreLimite: boolean;
  importarDatos: boolean;
}

export interface EstadoSeguridad {
  tienePin: boolean;
  protegidas: AccionesProtegidas;
}

/**
 * prueba: los primeros días, con todo. activa: licencia permanente. gracia: 7 días después de la
 * prueba, con todo y un aviso. consulta: sin licencia después de la gracia; se ve, exporta y
 * respalda, pero no se abre la caja ni se vende (lo decide Rust).
 */
export type ModoLicencia = "prueba" | "activa" | "gracia" | "consulta";

export interface DatosLicencia {
  id: string;
  negocio: string;
  rut: string | null;
  emitidaEn: string;
  equipos: number;
  /** ISO 8601 (UTC). */
  activadaEn: string | null;
}

export interface EstadoLicencia {
  modo: ModoLicencia;
  /** En prueba, los días que quedan; en gracia, los días de gracia que quedan. */
  diasRestantes: number | null;
  diasPrueba: number;
  inicioPrueba: string;
  finPrueba: string;
  finGracia: string;
  /** El reloj del equipo está más de 48 horas atrás: se avisa y no da días de prueba. */
  relojAtrasado: boolean;
  licencia: DatosLicencia | null;
  /** Hay una licencia guardada que no sirve en este equipo. */
  errorLicencia: string | null;
  codigoSolicitud: string;
  /** Cómo contactar al vendedor (se fija al compilar). */
  contacto: string;
  puedeVender: boolean;
}

/**
 * Error de un comando protegido: falta el PIN, es incorrecto o está bloqueado.
 * Los demás errores llegan como texto. Si el PIN es correcto lo decide siempre Rust.
 */
export interface ErrorPin {
  pin: "requerido" | "incorrecto" | "bloqueado";
  mensaje: string;
  /** Segundos que faltan para volver a probar, si está bloqueado. */
  segundos: number | null;
}

/** Billetes o monedas de una denominación contados al cerrar la caja. */
export interface Conteo {
  denominacion: number;
  cantidad: number;
}

/** Turno de caja: desde que se abre con un monto inicial hasta que se cierra contando el efectivo. */
export interface Turno {
  id: number;
  abiertoEn: string;
  cerradoEn: string | null;
  abierto: boolean;
  montoInicial: number;
  efectivoEsperado: number | null;
  efectivoContado: number | null;
  /** Contado − esperado: positivo sobra, negativo falta. */
  diferencia: number | null;
  detalleConteo: Conteo[] | null;
  notaCierre: string | null;
}

export interface EstadoCaja {
  /** null si la caja está cerrada. */
  turno: Turno | null;
  /** Monto inicial del último turno, para sugerirlo al abrir. */
  ultimoMontoInicial: number | null;
}

export type TipoMovimiento = "ingreso" | "retiro";

export interface MovimientoCaja {
  id: number;
  tipo: TipoMovimiento;
  monto: number;
  motivo: string;
  fecha: string;
}

/** Cantidad y monto de un medio de pago o de reembolso ("fiado" = sin reembolso). */
export interface TotalMedio {
  medio: MedioPago;
  cantidad: number;
  monto: number;
  redondeo: number;
}

export interface CuadreEfectivo {
  montoInicial: number;
  ventas: number;
  abonos: number;
  ingresos: number;
  retiros: number;
  reembolsos: number;
  esperado: number;
}

/** Resumen del turno; el efectivo esperado lo calcula Rust. */
export interface ResumenTurno {
  turno: Turno;
  cantidadVentas: number;
  totalVentas: number;
  ventas: TotalMedio[];
  redondeoVentas: number;
  abonos: TotalMedio[];
  anulaciones: TotalMedio[];
  devoluciones: TotalMedio[];
  /** Pagos de fiados anulados en el turno: su dinero salió de aquí. */
  pagosAnulados: TotalMedio[];
  redondeoReembolsos: number;
  movimientos: MovimientoCaja[];
  efectivo: CuadreEfectivo;
}

export interface TurnoBreve {
  id: number;
  abiertoEn: string;
  cerradoEn: string | null;
  abierto: boolean;
}

/** Estado que se busca en el historial ("conDevolucion" incluye las devueltas por completo). */
export type EstadoFiltro = "pagada" | "fiada" | "anulada" | "conDevolucion";
/** Estado de la insignia: "devuelta" es cuando se devolvió todo lo vendido. */
export type EstadoVenta = "pagada" | "fiada" | "anulada" | "conDevolucion" | "devuelta";

/** Filtros del historial. Fechas como días locales "aaaa-mm-dd" (hasta incluido). Con folio, el resto no se mira. */
export interface FiltrosVentas {
  desde: string | null;
  hasta: string | null;
  folio: number | null;
  medioPago: MedioPago | null;
  estado: EstadoFiltro | null;
  turnoId: number | null;
}

export interface VentaListada {
  id: number;
  folio: number;
  fechaHora: string;
  productos: number;
  total: number;
  medioPago: MedioPago;
  estado: EstadoVenta;
}

export interface PaginaVentas {
  ventas: VentaListada[];
  /** Solo en la primera página: cantidad y lo vendido (sin anuladas, descontando lo devuelto), en SQL. */
  totales: { cantidad: number; total: number } | null;
}

export interface LineaDetalle {
  /** Id de la línea vendida: el que se usa para devolver. */
  id: number;
  productoId: number;
  nombre: string;
  unidad: Unidad;
  precioUnitario: number;
  cantidadMil: number;
  subtotal: number;
  devueltoMil: number;
}

export interface Anulacion {
  fecha: string;
  motivo: string;
  turnoId: number | null;
  /** null en una venta fiada: no se devolvió dinero. */
  medioReembolso: MedioPago | null;
  reembolso: number | null;
}

export interface LineaDevuelta {
  ventaItemId: number;
  nombre: string;
  unidad: Unidad;
  precioUnitario: number;
  cantidadMil: number;
  monto: number;
  vuelveAStock: boolean;
}

export interface Devolucion {
  id: number;
  fecha: string;
  turnoId: number | null;
  /** Lo devuelto al cliente, con el redondeo incluido. */
  monto: number;
  redondeo: number;
  /** null en una venta fiada: bajó la deuda. */
  medioReembolso: MedioPago | null;
  motivo: string;
  lineas: LineaDevuelta[];
}

export interface DetalleVenta {
  id: number;
  folio: number;
  fechaHora: string;
  subtotal: number;
  descuento: number;
  redondeo: number;
  total: number;
  medioPago: MedioPago;
  montoRecibido: number | null;
  vuelto: number | null;
  cliente: string | null;
  estado: EstadoVenta;
  /** null en las ventas anteriores a los turnos. */
  turnoId: number | null;
  lineas: LineaDetalle[];
  anulacion: Anulacion | null;
  devoluciones: Devolucion[];
  montoDevuelto: number;
  tieneAbonos: boolean;
  /** Fiado: el cliente, lo abonado y lo que todavía debe de esta venta (lo calcula Rust). */
  clienteId: number | null;
  abonado: number;
  saldo: number | null;
}

/** Una línea a devolver; la cantidad en milésimas, como al vender. */
export interface ItemDevuelto {
  ventaItemId: number;
  cantidadMil: number;
  /** false: dañado o vencido, queda como merma. */
  vuelveAStock: boolean;
}

export interface DevolucionRegistrada {
  devolucionId: number;
  /** Lo devuelto (o lo que bajó la deuda), con el redondeo: lo calcula Rust. */
  monto: number;
  redondeo: number;
  venta: DetalleVenta;
}

/** Qué productos mostrar en Inventario > Stock. */
export type FiltroEstadoStock = "todos" | "bajoMinimo" | "sinStock" | "negativo";

export interface FiltrosStock {
  texto: string;
  categoriaId: number | null;
  sinCategoria: boolean;
  estado: FiltroEstadoStock;
}

export interface ProductoStock {
  id: number;
  codigoBarras: string | null;
  nombre: string;
  categoriaId: number | null;
  unidad: Unidad;
  precioVenta: number;
  costo: number | null;
  stockMil: number;
  stockMinimoMil: number;
  /** Stock × costo (lo calcula Rust); null si no tiene costo. */
  valor: number | null;
  bajoMinimo: boolean;
}

export interface ResumenInventario {
  valorCosto: number;
  valorVenta: number;
  /** Productos con stock y sin costo: cuentan como $0 en valorCosto. */
  sinCosto: number;
  bajoMinimo: number;
  productos: number;
}

export type TipoMovimientoStock = "venta" | "compra" | "ajuste" | "merma" | "devolucion";

/** Fechas como días locales "aaaa-mm-dd" (hasta incluido). */
export interface FiltrosMovimientos {
  productoId: number | null;
  tipo: TipoMovimientoStock | null;
  desde: string | null;
  hasta: string | null;
}

/** Dónde sigue la lista: fecha e id del último movimiento mostrado. */
export interface CursorMovimientos {
  fecha: string;
  id: number;
}

export interface MovimientoStock {
  id: number;
  fecha: string;
  productoId: number;
  producto: string;
  unidad: Unidad;
  tipo: TipoMovimientoStock;
  /** Positivo entra, negativo sale (milésimas). */
  cantidadMil: number;
  stockResultanteMil: number;
  motivo: string | null;
  nota: string | null;
  ventaId: number | null;
  folio: number | null;
  compraId: number | null;
}

export interface PaginaMovimientos {
  movimientos: MovimientoStock[];
  siguiente: CursorMovimientos | null;
}

export interface Contado {
  productoId: number;
  contadoMil: number;
}

/** Diferencia de un conteo contra el stock del momento (la calcula Rust). */
export interface Diferencia {
  productoId: number;
  nombre: string;
  unidad: Unidad;
  stockMil: number;
  contadoMil: number;
  diferenciaMil: number;
  /** La diferencia a costo; null si no tiene costo. */
  valor: number | null;
}

export type MotivoMerma = "vencido" | "danado" | "robo" | "consumoInterno" | "otro";

export interface Proveedor {
  id: number;
  nombre: string;
  rut: string | null;
  telefono: string | null;
  contacto: string | null;
  nota: string | null;
  activo: boolean;
  /** Compras vigentes (sin las anuladas) y la fecha de la última. */
  compras: number;
  ultimaCompra: string | null;
}

export type DatosProveedor = Pick<Proveedor, "nombre" | "rut" | "telefono" | "contacto" | "nota">;

/** Costo por unidad o, en productos por kg, por kilo. */
export interface LineaCompra {
  productoId: number;
  cantidadMil: number;
  costoUnitario: number;
}

export interface NuevaCompra {
  proveedorId: number | null;
  documento: string | null;
  nota: string | null;
  lineas: LineaCompra[];
  /** Se pagó con efectivo de la caja: un retiro en el turno abierto. */
  pagadaConCaja: boolean;
}

export interface Compra {
  id: number;
  fecha: string;
  proveedorId: number | null;
  proveedor: string | null;
  documento: string | null;
  total: number;
  nota: string | null;
  pagadaConCaja: boolean;
  anuladaEn: string | null;
  motivoAnulacion: string | null;
  lineas: number;
}

export interface ItemCompra {
  productoId: number;
  nombre: string;
  unidad: Unidad;
  cantidadMil: number;
  costoUnitario: number;
  subtotal: number;
  stockMil: number;
}

export interface DetalleCompra {
  compra: Compra;
  items: ItemCompra[];
  /** Productos que quedarían con stock negativo si se anulara ahora. */
  quedarianNegativos: string[];
}

/** Fechas como días locales "aaaa-mm-dd" (hasta incluido). */
export interface FiltrosCompras {
  proveedorId: number | null;
  desde: string | null;
  hasta: string | null;
}

export interface Cliente {
  id: number;
  nombre: string;
  telefono: string | null;
  rut: string | null;
  direccion: string | null;
  nota: string | null;
  /** null = sin límite. */
  limiteCredito: number | null;
  activo: boolean;
  creadoEn: string | null;
  /** Lo que debe hoy (lo calcula Rust). */
  deuda: number;
  /** Cuánto más se le puede fiar; null si no tiene límite. */
  disponible: number | null;
}

export type DatosCliente = Pick<Cliente, "nombre" | "telefono" | "rut" | "direccion" | "nota" | "limiteCredito">;

export interface Deudor {
  id: number;
  nombre: string;
  telefono: string | null;
  limiteCredito: number | null;
  activo: boolean;
  deuda: number;
  /** Fecha (UTC) de la fiada pendiente más antigua. */
  fiadaMasAntigua: string | null;
  ultimoPago: string | null;
}

export interface PaginaDeudores {
  clientes: Deudor[];
  /** Solo en la primera página. */
  totalAdeudado: number | null;
  clientesConDeuda: number | null;
}

/** Una venta fiada con lo que se debe de ella. */
export interface VentaFiada {
  ventaId: number;
  folio: number;
  fechaHora: string;
  total: number;
  /** Lo que bajaron sus devoluciones. */
  devuelto: number;
  abonado: number;
  saldo: number;
  pagadaEn: string | null;
}

export interface AbonoDePago {
  ventaId: number;
  folio: number;
  monto: number;
}

export interface Pago {
  id: number;
  clienteId: number;
  fecha: string;
  /** Lo que bajó la deuda; en efectivo entró monto + redondeo. */
  monto: number;
  redondeo: number;
  montoRecibido: number | null;
  vuelto: number | null;
  medioPago: MedioPago;
  turnoId: number | null;
  nota: string | null;
  anuladoEn: string | null;
  motivoAnulacion: string | null;
  abonos: AbonoDePago[];
}

export interface FichaCliente {
  cliente: Cliente;
  /** De la más antigua a la más nueva. */
  pendientes: VentaFiada[];
  pagadas: VentaFiada[];
  pagos: Pago[];
}

/** Sin ventas: de la más antigua a la más nueva; con ventas: primero esas. */
export interface NuevoPagoCliente {
  clienteId: number;
  monto: number;
  medioPago: Exclude<MedioPago, "fiado">;
  ventas: number[];
  /** Solo en efectivo; null = justo. */
  montoRecibido: number | null;
  nota: string | null;
}

export interface PagoRegistrado {
  pago: Pago;
  deudaRestante: number;
}

/** Etiqueta de balanza leída: producto por kg y los gramos de la etiqueta. */
export interface LecturaBalanza {
  producto: Producto;
  cantidadMil: number;
}

// --- Reportes ---------------------------------------------------------------------------
// Montos en pesos; porcentajes en puntos básicos (1234 = 12,34 %); días "aaaa-mm-dd" locales.

export interface Celda {
  monto: number;
  boletas: number;
}

export interface TotalMedioReporte {
  medio: MedioPago;
  monto: number;
  boletas: number;
}

export interface ProductoVendido {
  productoId: number;
  nombre: string;
  unidad: Unidad;
  categoriaId: number | null;
  categoria: string | null;
  cantidadMil: number;
  /** Ya sin lo devuelto. */
  monto: number;
  montoConCosto: number;
  /** null si ninguna de sus líneas tiene costo. */
  costo: number | null;
  ganancia: number | null;
  margenPb: number | null;
  /** Lo vendido sin costo conocido: no entra en la ganancia. */
  montoSinCosto: number;
  /** Parte del costo es estimada (ventas de antes de guardar el costo en cada línea). */
  costoEstimado: boolean;
}

export interface BajoMinimo {
  id: number;
  nombre: string;
  unidad: Unidad;
  stockMil: number;
  stockMinimoMil: number;
}

export interface ClienteAtrasado {
  id: number;
  nombre: string;
  deuda: number;
  desde: string;
  dias: number;
}

export interface ResumenHoy {
  fecha: string;
  ventas: number;
  boletas: number;
  ticketPromedio: number;
  ganancia: number;
  ventasSinCosto: number;
  /** El mismo día de la semana pasada hasta la misma hora. */
  ventasSemanaAnterior: number;
  variacionPb: number | null;
  /** null con la caja cerrada. */
  efectivoEsperado: number | null;
  fiadoOtorgado: number;
  pagosFiado: number;
  /** 24 horas. */
  porHora: Celda[];
  porMedio: TotalMedioReporte[];
  masVendidos: ProductoVendido[];
  bajoMinimo: BajoMinimo[];
  totalBajoMinimo: number;
  atrasados: ClienteAtrasado[];
  diasAlerta: number;
}

export interface ReporteVentas {
  desde: string;
  hasta: string;
  total: number;
  boletas: number;
  ticketPromedio: number;
  porDia: { dia: string; monto: number; boletas: number }[];
  porMedio: TotalMedioReporte[];
  porCategoria: { categoriaId: number | null; nombre: string; monto: number }[];
  /** Lunes a domingo. */
  porDiaSemana: Celda[];
  /** 7 × 24: lunes primero, por hora local. */
  mapa: Celda[][];
}

export interface ReporteProductos {
  desde: string;
  hasta: string;
  productos: ProductoVendido[];
  sinVentas: { productoId: number; nombre: string; unidad: Unidad; categoria: string | null; stockMil: number }[];
}

export interface ReporteGanancia {
  desde: string;
  hasta: string;
  ventas: number;
  costo: number;
  ganancia: number;
  margenPb: number | null;
  ventasEstimadas: number;
  costoEstimado: number;
  ventasSinCosto: number;
  productos: ProductoVendido[];
  porCategoria: { categoriaId: number | null; nombre: string; ventas: number; costo: number; ganancia: number; margenPb: number | null }[];
  sinCosto: ProductoVendido[];
}

export interface ReporteTurnos {
  desde: string;
  hasta: string;
  turnos: { id: number; abiertoEn: string; cerradoEn: string; montoInicial: number; esperado: number; contado: number; diferencia: number }[];
  sobrantes: number;
  faltantes: number;
  cuadrados: number;
}

export interface ReporteFiados {
  desde: string;
  hasta: string;
  deudaAlCierre: number;
  clientesConDeuda: number;
  otorgado: number;
  pagado: number;
  antiguedad: { tramo: string; monto: number; ventas: number }[];
  deudores: { clienteId: number; nombre: string; deuda: number }[];
}

export type TipoReporte = "ventas" | "productos" | "ganancia" | "turnos" | "fiados";
export type FormatoExportacion = "excel" | "pdf";

// --- Importar y exportar Excel ----------------------------------------------------------

export type CampoProducto = "codigo" | "nombre" | "categoria" | "unidad" | "precio" | "costo" | "stock" | "stockMinimo";
export type CampoCliente = "nombre" | "telefono" | "rut" | "direccion" | "nota" | "limite";
export type TipoImportacion = "productos" | "clientes";
export type Exportacion = "productos" | "stock" | "ventas" | "movimientos" | "compras" | "clientes";

/** La planilla elegida y lo que se detectó en cada columna. */
export interface Apertura {
  ruta: string;
  encabezados: string[];
  productos: (CampoProducto | null)[];
  clientes: (CampoCliente | null)[];
  filas: number;
  /** Las primeras filas como texto. */
  muestra: string[][];
}

export interface FilaError {
  fila: number;
  motivo: string;
}

export interface FilaProductoImportado {
  fila: number;
  codigo: string | null;
  nombre: string;
  categoria: string | null;
  unidad: Unidad;
  precio: number;
  costo: number | null;
  stockMil: number | null;
  stockMinimoMil: number;
  cambios: string[];
}

export interface FilaClienteImportado {
  fila: number;
  nombre: string;
  telefono: string | null;
  rut: string | null;
  direccion: string | null;
  nota: string | null;
  limiteCredito: number | null;
  cambios: string[];
}

export interface VistaProductos {
  huella: string;
  nuevas: FilaProductoImportado[];
  actualizan: FilaProductoImportado[];
  errores: FilaError[];
  sinCambios: number;
  categoriasNuevas: string[];
}

export interface VistaClientes {
  huella: string;
  nuevas: FilaClienteImportado[];
  actualizan: FilaClienteImportado[];
  errores: FilaError[];
  sinCambios: number;
}

export interface ResultadoImportacion {
  creados: number;
  actualizados: number;
  omitidos: number;
  categoriasCreadas: number;
}

// --- Respaldos ---------------------------------------------------------------------------

export interface Respaldo {
  nombre: string;
  ruta: string;
  bytes: number;
  /** Cuándo se escribió (UTC). */
  fecha: string;
}

export interface EstadoRespaldos {
  rutaBase: string;
  bytesBase: number;
  carpeta: string;
  carpetaPredeterminada: string;
  conservar: number;
  /** Del más nuevo al más antiguo. */
  respaldos: Respaldo[];
  /** El último respaldo automático que falló. */
  error: string | null;
}

/** Lo que se muestra antes de restaurar. */
export interface InfoRespaldo {
  ruta: string;
  bytes: number;
  fecha: string;
  versionEsquema: number;
  versionApp: number;
  negocio: string | null;
  productos: number;
  ventas: number;
  ultimaVenta: string | null;
}

export interface EstadoIntegridad {
  /** La vez anterior no se cerró bien. */
  cierreBrusco: boolean;
  /** Lo que dijo la revisión de la base; vacío si está sana (o no hacía falta revisarla). */
  problemas: string[];
  /** El respaldo más nuevo que pasa la verificación. */
  ultimoSano: { nombre: string; ruta: string; bytes: number; fecha: string } | null;
}

export interface DatosActualizacion {
  version: string;
  /** Las notas de la versión (su apartado del CHANGELOG). */
  notas: string | null;
  fecha: string | null;
}

export interface EstadoActualizacion {
  versionActual: string;
  disponible: DatosActualizacion | null;
  /** Segundos desde 1970 de la última búsqueda que llegó al servidor. */
  ultimaBusqueda: number | null;
  instalarAlCerrar: boolean;
}

/** Rust avisa cuando la búsqueda automática encuentra una versión nueva. */
export function escucharActualizacion(alEncontrar: () => void): () => void {
  let dejar: (() => void) | null = null;
  let cancelado = false;
  void listen("actualizacion-disponible", () => alEncontrar()).then((f) => {
    if (cancelado) f();
    else dejar = f;
  });
  return () => {
    cancelado = true;
    dejar?.();
  };
}

/** Rust avisa si un respaldo automático falló (con el mensaje). */
export function escucharRespaldoFallido(alFallar: (mensaje: string) => void): () => void {
  let dejar: (() => void) | null = null;
  let cancelado = false;
  void listen<string>("respaldo-fallido", (e) => alFallar(e.payload)).then((f) => {
    if (cancelado) f();
    else dejar = f;
  });
  return () => {
    cancelado = true;
    dejar?.();
  };
}

/** Rust avisa cuando cambia algo de la pantalla Hoy (ventas, caja, fiados, stock). */
export function escucharCambiosDelDia(alCambiar: () => void): () => void {
  let dejar: (() => void) | null = null;
  let cancelado = false;
  void listen("cambios-del-dia", () => alCambiar()).then((f) => {
    if (cancelado) f();
    else dejar = f;
  });
  return () => {
    cancelado = true;
    dejar?.();
  };
}

export const api = {
  estadoSistema: () => invoke<EstadoSistema>("estado_sistema"),

  obtenerNegocio: () => invoke<Negocio>("obtener_negocio"),
  // Los comandos que reciben `pin` están protegidos: sin PIN o con uno incorrecto fallan con ErrorPin.
  guardarNegocio: (datos: DatosNegocio, pin: string | null) => invoke<Negocio>("guardar_negocio", { datos, pin }),
  /** Abre el diálogo de archivos; null si el usuario cancela. */
  elegirLogo: (pin: string | null) => invoke<Negocio | null>("elegir_logo", { pin }),
  quitarLogo: (pin: string | null) => invoke<Negocio>("quitar_logo", { pin }),

  /** Guarda el PDF de la boleta y, si `abrir`, lo abre. Devuelve la ruta. */
  guardarBoletaPdf: (ventaId: number, abrir: boolean) =>
    invoke<string>("guardar_boleta_pdf", { ventaId, abrir }),
  carpetaPdfPredeterminada: () => invoke<string>("carpeta_pdf_predeterminada"),
  /** Diálogo de carpeta; null si el usuario cancela. */
  elegirCarpetaPdf: () => invoke<string | null>("elegir_carpeta_pdf"),

  listarImpresoras: () => invoke<Impresora[]>("listar_impresoras"),
  estadoImpresora: () => invoke<EstadoConfigurada>("estado_impresora"),
  /** Sin `copias`, las de la configuración. Las reimpresiones (`copia`) dicen "COPIA". */
  imprimirBoleta: (ventaId: number, copias?: number, copia = false) =>
    invoke<void>("imprimir_boleta", { ventaId, copias: copias ?? null, copia }),
  imprimirPrueba: () => invoke<void>("imprimir_prueba"),
  abrirCajon: () => invoke<void>("abrir_cajon"),
  carpetaImpresoraPruebas: () => invoke<string>("carpeta_impresora_pruebas"),
  ultimaVenta: () => invoke<number | null>("ultima_venta"),

  buscarProductoPorCodigo: (codigo: string) =>
    invoke<Producto | null>("buscar_producto_por_codigo", { codigo }),
  buscarProductos: (
    texto: string,
    incluirInactivos: boolean,
    limite: number,
    desplazamiento: number,
    categoria: FiltroCategoria = "todas",
  ) =>
    invoke<Producto[]>("buscar_productos", {
      texto,
      incluirInactivos,
      categoriaId: typeof categoria === "number" ? categoria : null,
      sinCategoria: categoria === "sin",
      limite,
      desplazamiento,
    }),
  /** null si no es una etiqueta de balanza o los códigos de balanza están desactivados. */
  leerCodigoBalanza: (codigo: string) => invoke<LecturaBalanza | null>("leer_codigo_balanza", { codigo }),
  crearProducto: (datos: DatosProducto) => invoke<Producto>("crear_producto", { datos }),
  editarProducto: (id: number, datos: DatosProducto) =>
    invoke<Producto>("editar_producto", { id, datos }),
  cambiarEstadoProducto: (id: number, activo: boolean) =>
    invoke<void>("cambiar_estado_producto", { id, activo }),

  listarCategorias: () => invoke<Categoria[]>("listar_categorias"),
  crearCategoria: (nombre: string) => invoke<Categoria>("crear_categoria", { nombre }),
  renombrarCategoria: (id: number, nombre: string) =>
    invoke<Categoria>("renombrar_categoria", { id, nombre }),
  eliminarCategoria: (id: number) => invoke<void>("eliminar_categoria", { id }),

  /** Una fiada sobre el límite se rechaza salvo con `sobreLimite` (pide PIN si está protegido). */
  confirmarVenta: (venta: NuevaVenta, sobreLimite = false, pin: string | null = null) =>
    invoke<VentaConfirmada>("confirmar_venta", { venta, sobreLimite, pin }),

  buscarClientes: (texto: string, limite: number) => invoke<Cliente[]>("buscar_clientes", { texto, limite }),
  obtenerCliente: (id: number) => invoke<Cliente>("obtener_cliente", { id }),
  crearCliente: (datos: DatosCliente) => invoke<Cliente>("crear_cliente", { datos }),
  editarCliente: (id: number, datos: DatosCliente) => invoke<Cliente>("editar_cliente", { id, datos }),
  /** Desactivar solo si no debe nada. */
  cambiarEstadoCliente: (id: number, activo: boolean) => invoke<Cliente>("cambiar_estado_cliente", { id, activo }),
  reporteHoy: () => invoke<ResumenHoy>("reporte_hoy"),
  reporteVentas: (desde: string, hasta: string) => invoke<ReporteVentas>("reporte_ventas", { desde, hasta }),
  /** categoria: null todas, 0 sin categoría. */
  reporteProductos: (desde: string, hasta: string, categoria: number | null) =>
    invoke<ReporteProductos>("reporte_productos", { desde, hasta, categoria }),
  reporteGanancia: (desde: string, hasta: string) => invoke<ReporteGanancia>("reporte_ganancia", { desde, hasta }),
  reporteTurnos: (desde: string, hasta: string) => invoke<ReporteTurnos>("reporte_turnos", { desde, hasta }),
  reporteFiados: (desde: string, hasta: string) => invoke<ReporteFiados>("reporte_fiados", { desde, hasta }),
  /** Pregunta dónde guardar; null si se cancela. */
  exportarReporte: (tipo: TipoReporte, desde: string, hasta: string, categoria: number | null, formato: FormatoExportacion) =>
    invoke<string | null>("exportar_reporte", { tipo, desde, hasta, categoria, formato }),
  abrirArchivo: (ruta: string) => invoke<void>("abrir_archivo", { ruta }),
  estadoRespaldos: () => invoke<EstadoRespaldos>("estado_respaldos"),
  respaldarAhora: () => invoke<Respaldo>("respaldar_ahora"),
  /** Otra carpeta (un pendrive); null si se cancela. */
  respaldarEn: () => invoke<Respaldo | null>("respaldar_en"),
  elegirCarpetaRespaldos: () => invoke<string | null>("elegir_carpeta_respaldos"),
  abrirCarpetaRespaldos: () => invoke<void>("abrir_carpeta_respaldos"),
  /** null si se cancela. */
  elegirRespaldo: () => invoke<string | null>("elegir_respaldo"),
  inspeccionarRespaldo: (ruta: string) => invoke<InfoRespaldo>("inspeccionar_respaldo", { ruta }),
  /** Siempre pide el PIN (si hay uno). Si sale bien, la app se reinicia. */
  restaurarRespaldo: (ruta: string, pin: string | null) => invoke<void>("restaurar_respaldo", { ruta, pin }),
  elegirPlanilla: () => invoke<string | null>("elegir_planilla"),
  abrirPlanilla: (ruta: string) => invoke<Apertura>("abrir_planilla", { ruta }),
  previsualizarProductos: (ruta: string, mapeo: (CampoProducto | null)[]) =>
    invoke<VistaProductos>("previsualizar_productos", { ruta, mapeo }),
  previsualizarClientes: (ruta: string, mapeo: (CampoCliente | null)[]) =>
    invoke<VistaClientes>("previsualizar_clientes", { ruta, mapeo }),
  /** Protegido con el PIN de "importar datos". */
  importarProductos: (ruta: string, mapeo: (CampoProducto | null)[], huella: string, pin: string | null) =>
    invoke<ResultadoImportacion>("importar_productos", { ruta, mapeo, huella, pin }),
  importarClientes: (ruta: string, mapeo: (CampoCliente | null)[], huella: string, pin: string | null) =>
    invoke<ResultadoImportacion>("importar_clientes", { ruta, mapeo, huella, pin }),
  descargarPlanillaEjemplo: (tipo: TipoImportacion) => invoke<string | null>("descargar_planilla_ejemplo", { tipo }),
  /** Ventas, movimientos y compras usan el rango. null si se cancela el "Guardar como". */
  exportarDatos: (que: Exportacion, desde: string | null, hasta: string | null) =>
    invoke<string | null>("exportar_datos", { que, desde, hasta }),
  listarDeudores: (texto: string, todos: boolean, limite: number, desplazamiento: number) =>
    invoke<PaginaDeudores>("listar_deudores", { texto, todos, limite, desplazamiento }),
  fichaCliente: (id: number) => invoke<FichaCliente>("ficha_cliente", { id }),
  registrarPago: (pago: NuevoPagoCliente) => invoke<PagoRegistrado>("registrar_pago", { pago }),
  /** Protegido con el PIN de anular venta. */
  anularPago: (id: number, motivo: string, pin: string | null) =>
    invoke<PagoRegistrado>("anular_pago", { id, motivo, pin }),
  imprimirPago: (pagoId: number) => invoke<void>("imprimir_pago", { pagoId }),
  imprimirEstadoCuenta: (clienteId: number) => invoke<void>("imprimir_estado_cuenta", { clienteId }),
  guardarEstadoCuentaPdf: (clienteId: number, abrir: boolean) =>
    invoke<string>("guardar_estado_cuenta_pdf", { clienteId, abrir }),
  listarVentas: (filtros: FiltrosVentas, limite: number, desplazamiento: number) =>
    invoke<PaginaVentas>("listar_ventas", { filtros, limite, desplazamiento }),
  detalleVenta: (id: number) => invoke<DetalleVenta>("detalle_venta", { id }),
  /** Id de la venta con ese folio (escrito o escaneado de la boleta); null si no existe. */
  ventaPorFolio: (folio: number) => invoke<number | null>("venta_por_folio", { folio }),
  /** Anula la venta; medioReembolso null en una fiada (no se devuelve dinero). Protegido con PIN. */
  anularVenta: (id: number, motivo: string, medioReembolso: MedioPago | null, pin: string | null) =>
    invoke<DetalleVenta>("anular_venta", { id, motivo, medioReembolso, pin }),
  imprimirAnulacion: (ventaId: number) => invoke<void>("imprimir_anulacion", { ventaId }),
  /** Devuelve parte de una venta; medioReembolso null en una fiada. Protegido con PIN. */
  registrarDevolucion: (
    ventaId: number,
    items: ItemDevuelto[],
    medioReembolso: MedioPago | null,
    motivo: string,
    pin: string | null,
  ) => invoke<DevolucionRegistrada>("registrar_devolucion", { ventaId, items, medioReembolso, motivo, pin }),
  imprimirDevolucion: (devolucionId: number) => invoke<void>("imprimir_devolucion", { devolucionId }),

  obtenerConfig: () => invoke<Config>("obtener_config"),
  guardarConfig: (config: Config, pin: string | null) => invoke<Config>("guardar_config", { config, pin }),

  listarStock: (filtros: FiltrosStock, limite: number, desplazamiento: number) =>
    invoke<ProductoStock[]>("listar_stock", { filtros, limite, desplazamiento }),
  resumenInventario: () => invoke<ResumenInventario>("resumen_inventario"),
  contarBajoMinimo: () => invoke<number>("contar_bajo_minimo"),
  listarMovimientos: (filtros: FiltrosMovimientos, despuesDe: CursorMovimientos | null, limite: number) =>
    invoke<PaginaMovimientos>("listar_movimientos", { filtros, despuesDe, limite }),
  previsualizarConteo: (items: Contado[]) => invoke<Diferencia[]>("previsualizar_conteo", { items }),
  /** Deja el stock en lo contado, todo en una transacción. Protegido con PIN. */
  ajustarConteo: (items: Contado[], motivo: string, pin: string | null) =>
    invoke<Diferencia[]>("ajustar_conteo", { items, motivo, pin }),
  /** Devuelve el stock que quedó. Protegido con PIN. */
  registrarMerma: (
    productoId: number,
    cantidadMil: number,
    motivo: MotivoMerma,
    detalle: string | null,
    pin: string | null,
  ) => invoke<number>("registrar_merma", { productoId, cantidadMil, motivo, detalle, pin }),

  listarProveedores: (texto: string, incluirInactivos: boolean, limite: number, desplazamiento: number) =>
    invoke<Proveedor[]>("listar_proveedores", { texto, incluirInactivos, limite, desplazamiento }),
  crearProveedor: (datos: DatosProveedor) => invoke<Proveedor>("crear_proveedor", { datos }),
  editarProveedor: (id: number, datos: DatosProveedor) => invoke<Proveedor>("editar_proveedor", { id, datos }),
  cambiarEstadoProveedor: (id: number, activo: boolean) =>
    invoke<Proveedor>("cambiar_estado_proveedor", { id, activo }),
  /** Si se paga con la caja, pide el PIN de los retiros cuando está protegido. */
  registrarCompra: (compra: NuevaCompra, pin: string | null) =>
    invoke<DetalleCompra>("registrar_compra", { compra, pin }),
  detalleCompra: (id: number) => invoke<DetalleCompra>("detalle_compra", { id }),
  listarCompras: (filtros: FiltrosCompras, limite: number, desplazamiento: number) =>
    invoke<Compra[]>("listar_compras", { filtros, limite, desplazamiento }),
  /** Compensa stock y caja. Protegido con el PIN de ajustar inventario. */
  anularCompra: (id: number, motivo: string, pin: string | null) =>
    invoke<DetalleCompra>("anular_compra", { id, motivo, pin }),

  estadoCaja: () => invoke<EstadoCaja>("estado_caja"),
  abrirTurno: (montoInicial: number) => invoke<Turno>("abrir_turno", { montoInicial }),
  registrarMovimientoCaja: (tipo: TipoMovimiento, monto: number, motivo: string, pin: string | null) =>
    invoke<MovimientoCaja>("registrar_movimiento_caja", { tipo, monto, motivo, pin }),
  resumenTurno: (turnoId: number) => invoke<ResumenTurno>("resumen_turno", { turnoId }),
  cerrarTurno: (efectivoContado: number, detalleConteo: Conteo[] | null, nota: string | null, pin: string | null) =>
    invoke<ResumenTurno>("cerrar_turno", { efectivoContado, detalleConteo, nota, pin }),
  listarTurnos: (limite: number, desplazamiento: number) =>
    invoke<TurnoBreve[]>("listar_turnos", { limite, desplazamiento }),
  imprimirCierre: (turnoId: number) => invoke<void>("imprimir_cierre", { turnoId }),
  /** Guarda el PDF del cierre y, si `abrir`, lo abre. Devuelve la ruta. */
  guardarCierrePdf: (turnoId: number, abrir: boolean) => invoke<string>("guardar_cierre_pdf", { turnoId, abrir }),

  estadoIntegridad: () => invoke<EstadoIntegridad>("estado_integridad"),
  /** Restaura el último respaldo sano (guarda antes la base dañada) y reinicia la app. */
  restaurarUltimoSano: (pin: string | null) => invoke<void>("restaurar_ultimo_sano", { pin }),
  abrirCarpetaRegistros: () => invoke<void>("abrir_carpeta_registros"),
  /** Texto para pedir soporte: versión, Windows, licencia (sin la clave), esquema e impresora. */
  informacionSoporte: () => invoke<string>("informacion_soporte"),
  estadoActualizacion: () => invoke<EstadoActualizacion>("estado_actualizacion"),
  /** Busca ahora (sin esperar el día). Falla con un mensaje si no hay internet. */
  buscarActualizacion: () => invoke<EstadoActualizacion>("buscar_actualizacion"),
  /** Con un turno abierto: la vista instala sola cuando se cierre. */
  instalarAlCerrarTurno: (si: boolean) => invoke<EstadoActualizacion>("instalar_al_cerrar_turno", { si }),
  /** Respalda, descarga la actualización firmada y la instala; la app se cierra. Rust la rechaza con un turno abierto. */
  instalarActualizacion: () => invoke<void>("instalar_actualizacion"),
  /** La base está vacía y el asistente de primer arranque no se terminó. */
  asistentePendiente: () => invoke<boolean>("asistente_pendiente"),
  terminarAsistente: () => invoke<void>("terminar_asistente"),
  estadoLicencia: () => invoke<EstadoLicencia>("estado_licencia"),
  /** El texto pegado o dictado (CHUPS-XXXXX-…). Rust revisa la firma y el equipo. */
  activarLicencia: (texto: string) => invoke<EstadoLicencia>("activar_licencia", { texto }),
  /** Elige el archivo .chups-licencia y lo activa; null si se canceló. */
  cargarArchivoLicencia: () => invoke<EstadoLicencia | null>("cargar_archivo_licencia"),

  estadoSeguridad: () => invoke<EstadoSeguridad>("estado_seguridad"),
  /** Devuelve el código de recuperación, que se muestra una sola vez. */
  crearPin: (pin: string) => invoke<string>("crear_pin", { pin }),
  cambiarPin: (actual: string, nuevo: string) => invoke<void>("cambiar_pin", { actual, nuevo }),
  quitarPin: (pin: string | null) => invoke<void>("quitar_pin", { pin }),
  /** PIN nuevo con el código de recuperación; devuelve el código nuevo. */
  recuperarPin: (codigo: string, nuevo: string) => invoke<string>("recuperar_pin", { codigo, nuevo }),
  guardarProtegidas: (protegidas: AccionesProtegidas, pin: string | null) =>
    invoke<EstadoSeguridad>("guardar_protegidas", { protegidas, pin }),

  ponerEnEspera: (nombre: string | null, items: ItemPedido[]) =>
    invoke<number>("poner_en_espera", { nombre, items }),
  listarEnEspera: () => invoke<ResumenEspera[]>("listar_en_espera"),
  /** Con `intercambio`, el carrito actual queda en espera en la misma transacción. */
  recuperarEnEspera: (id: number, intercambio: { nombre: string | null; items: ItemPedido[] } | null) =>
    invoke<VentaRecuperada>("recuperar_en_espera", { id, intercambio }),
  descartarEnEspera: (id: number) => invoke<void>("descartar_en_espera", { id }),
};

/** El error de PIN de un comando protegido, o null si el error es otro. */
export const errorPin = (e: unknown): ErrorPin | null =>
  typeof e === "object" && e !== null && "pin" in e && "mensaje" in e ? (e as ErrorPin) : null;

/** Los errores de Rust llegan como texto listo para mostrar (los del PIN, con su mensaje). */
export const textoError = (e: unknown) => (typeof e === "string" ? e : (errorPin(e)?.mensaje ?? String(e)));
