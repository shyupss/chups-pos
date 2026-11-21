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

export interface Producto {
  id: number;
  codigoBarras: string | null;
  nombre: string;
  precioVenta: number;
  costo: number | null;
  stock: number;
  stockMinimo: number;
  activo: boolean;
}

export interface DatosProducto {
  codigoBarras: string | null;
  nombre: string;
  precioVenta: number;
  costo: number | null;
  stockMinimo: number;
  /** Solo se usa al crear. */
  stockInicial: number;
}

export type MedioPago = "efectivo" | "debito" | "credito" | "transferencia" | "fiado";

export interface NuevaVenta {
  items: { productoId: number; cantidad: number }[];
  medioPago: MedioPago;
  montoRecibido: number | null;
  descuento: number;
  clienteId: number | null;
}

export interface LineaVendida {
  productoId: number;
  nombre: string;
  precioUnitario: number;
  cantidad: number;
  subtotal: number;
  stockResultante: number;
}

export interface VentaConfirmada {
  id: number;
  folio: number;
  fechaHora: string;
  subtotal: number;
  descuento: number;
  total: number;
  medioPago: MedioPago;
  montoRecibido: number | null;
  vuelto: number | null;
  lineas: LineaVendida[];
}

export const api = {
  estadoSistema: () => invoke<EstadoSistema>("estado_sistema"),

  buscarProductoPorCodigo: (codigo: string) =>
    invoke<Producto | null>("buscar_producto_por_codigo", { codigo }),
  buscarProductos: (texto: string, incluirInactivos: boolean, limite: number, desplazamiento: number) =>
    invoke<Producto[]>("buscar_productos", { texto, incluirInactivos, limite, desplazamiento }),
  crearProducto: (datos: DatosProducto) => invoke<Producto>("crear_producto", { datos }),
  editarProducto: (id: number, datos: DatosProducto) =>
    invoke<Producto>("editar_producto", { id, datos }),
  cambiarEstadoProducto: (id: number, activo: boolean) =>
    invoke<void>("cambiar_estado_producto", { id, activo }),

  confirmarVenta: (venta: NuevaVenta) => invoke<VentaConfirmada>("confirmar_venta", { venta }),
};

/** Los errores de Rust llegan como texto listo para mostrar. */
export const textoError = (e: unknown) => (typeof e === "string" ? e : String(e));
