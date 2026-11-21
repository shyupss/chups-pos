// Única puerta de la Vista hacia el Controlador (comandos Tauri en Rust).
// Cada comando tiene aquí su función tipada; las pantallas no llaman a `invoke` directo.
import { invoke } from "@tauri-apps/api/core";

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
}

export type TablaCaracteres = "cp858" | "cp850" | "wpc1252";
export type FormatoPdf = "ticket" | "carta";
export type FormatoBalanza = "peso" | "precio";

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
}

export interface EstadoSeguridad {
  tienePin: boolean;
  protegidas: AccionesProtegidas;
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

/** Etiqueta de balanza leída: producto por kg y los gramos de la etiqueta. */
export interface LecturaBalanza {
  producto: Producto;
  cantidadMil: number;
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

  confirmarVenta: (venta: NuevaVenta) => invoke<VentaConfirmada>("confirmar_venta", { venta }),
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
