import type { EstadoVenta } from "../api";
import Insignia, { type TonoInsignia } from "./Insignia";

const ESTILO: Record<EstadoVenta, [string, TonoInsignia]> = {
  pagada: ["Pagada", "exito"],
  fiada: ["Fiada", "info"],
  anulada: ["Anulada", "peligro"],
  conDevolucion: ["Con devolución", "aviso"],
  devuelta: ["Devuelta", "neutro"],
};

/** Estado de una venta en la lista y en el detalle. */
export default function InsigniaEstado({ estado }: { estado: EstadoVenta }) {
  const [texto, tono] = ESTILO[estado];
  return <Insignia tono={tono}>{texto}</Insignia>;
}
