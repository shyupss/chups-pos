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

export interface ConfigCaja {
  redondeoEfectivo: boolean;
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

export const api = {
  estadoSistema: () => invoke<EstadoSistema>("estado_sistema"),

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

  configCaja: () => invoke<ConfigCaja>("config_caja"),
  cambiarRedondeoEfectivo: (activo: boolean) => invoke<void>("cambiar_redondeo_efectivo", { activo }),

  ponerEnEspera: (nombre: string | null, items: ItemPedido[]) =>
    invoke<number>("poner_en_espera", { nombre, items }),
  listarEnEspera: () => invoke<ResumenEspera[]>("listar_en_espera"),
  /** Con `intercambio`, el carrito actual queda en espera en la misma transacción. */
  recuperarEnEspera: (id: number, intercambio: { nombre: string | null; items: ItemPedido[] } | null) =>
    invoke<VentaRecuperada>("recuperar_en_espera", { id, intercambio }),
  descartarEnEspera: (id: number) => invoke<void>("descartar_en_espera", { id }),
};

/** Los errores de Rust llegan como texto listo para mostrar. */
export const textoError = (e: unknown) => (typeof e === "string" ? e : String(e));
