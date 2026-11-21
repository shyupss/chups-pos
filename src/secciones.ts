import {
  Boxes,
  ChartColumn,
  NotebookPen,
  Package,
  Receipt,
  Settings,
  ShoppingCart,
  type LucideIcon,
} from "lucide-react";

export type Seccion =
  | "caja"
  | "productos"
  | "inventario"
  | "fiados"
  | "ventas"
  | "reportes"
  | "configuracion";

export interface DatosSeccion {
  id: Seccion;
  nombre: string;
  tecla: string;
  Icono: LucideIcon;
  /** Va separada al pie de la barra lateral. */
  abajo?: boolean;
  /** Mientras la sección no exista, su pantalla muestra solo este texto. */
  pendiente?: string;
}

// El orden es el de la barra lateral. Las teclas son atajos globales.
export const SECCIONES: DatosSeccion[] = [
  { id: "caja", nombre: "Caja", tecla: "F1", Icono: ShoppingCart },
  { id: "productos", nombre: "Productos", tecla: "F2", Icono: Package },
  {
    id: "inventario",
    nombre: "Inventario",
    tecla: "F3",
    Icono: Boxes,
    pendiente: "Ingreso de mercadería, ajustes, mermas, historial por producto y proveedores. Llega en la fase 5.",
  },
  {
    id: "fiados",
    nombre: "Fiados",
    tecla: "F4",
    Icono: NotebookPen,
    pendiente: "Clientes, deudas y registro de pagos parciales. Llega en la fase 5.",
  },
  {
    id: "ventas",
    nombre: "Ventas",
    tecla: "F5",
    Icono: Receipt,
    pendiente: "Historial de boletas, reimpresión, anulación y devoluciones. Llega en la fase 4.",
  },
  {
    id: "reportes",
    nombre: "Reportes",
    tecla: "F6",
    Icono: ChartColumn,
    pendiente: "Pantalla Hoy, ventas por día o rango, más vendidos, ganancia y stock bajo. Llega en la fase 6.",
  },
  { id: "configuracion", nombre: "Configuración", tecla: "F9", Icono: Settings, abajo: true },
];
