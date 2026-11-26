// La lista de atajos que muestra la ayuda ("Atajos" al pie de la barra lateral). Si cambia un
// atajo en el código, cambia aquí también.

import { SECCIONES } from "./secciones";

export interface GrupoAtajos {
  titulo: string;
  atajos: { teclas: string[]; que: string }[];
}

export const GRUPOS_ATAJOS: GrupoAtajos[] = [
  {
    titulo: "Secciones",
    atajos: SECCIONES.map((s) => ({ teclas: [s.tecla], que: `Ir a ${s.nombre}` })),
  },
  {
    titulo: "Caja",
    atajos: [
      { teclas: ["F12"], que: "Cobrar" },
      { teclas: ["F8"], que: "Poner la venta en espera" },
      { teclas: ["F7"], que: "Ver las ventas en espera" },
      { teclas: ["Ctrl", "P"], que: "Reimprimir la última boleta" },
      { teclas: ["Esc"], que: "Limpiar la búsqueda" },
    ],
  },
  {
    titulo: "Cobro",
    atajos: [
      { teclas: ["1"], que: "Efectivo" },
      { teclas: ["2"], que: "Débito" },
      { teclas: ["3"], que: "Crédito" },
      { teclas: ["4"], que: "Transferencia" },
      { teclas: ["5"], que: "Fiado" },
      { teclas: ["Alt", "1–5"], que: "Cambiar el medio de pago mientras escribes un monto" },
      { teclas: ["Enter"], que: "Confirmar la venta" },
      { teclas: ["F12"], que: "Confirmar el fiado" },
      { teclas: ["Esc"], que: "Volver a la venta" },
    ],
  },
  {
    titulo: "Reportes",
    atajos: [{ teclas: ["Ctrl", "E"], que: "Descargar el reporte en Excel" }],
  },
  {
    titulo: "Otras pantallas",
    atajos: [
      { teclas: ["Ctrl", "N"], que: "Inventario: registrar una compra" },
      { teclas: ["Ctrl", "P"], que: "Ventas: reimprimir la boleta abierta · Fiados: registrar un pago" },
      { teclas: ["Esc"], que: "Cerrar un diálogo o el detalle de una venta" },
    ],
  },
];
