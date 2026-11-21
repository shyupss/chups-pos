import type { EstadoVenta } from "../api";

const ESTILO: Record<EstadoVenta, [string, string]> = {
  pagada: ["Pagada", "bg-exito-fondo text-exito"],
  fiada: ["Fiada", "bg-info-fondo text-info"],
  anulada: ["Anulada", "bg-peligro-fondo text-peligro"],
  conDevolucion: ["Con devolución", "bg-aviso-fondo text-aviso"],
  devuelta: ["Devuelta", "bg-suave text-tenue"],
};

/** Estado de una venta en la lista y en el detalle. */
export default function InsigniaEstado({ estado }: { estado: EstadoVenta }) {
  const [texto, clase] = ESTILO[estado];
  return (
    <span className={`inline-flex h-6 items-center rounded-full px-2.5 text-xs font-medium whitespace-nowrap ${clase}`}>
      {texto}
    </span>
  );
}
