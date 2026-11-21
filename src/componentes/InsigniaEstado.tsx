import type { EstadoVenta } from "../api";

const ESTILO: Record<EstadoVenta, [string, string]> = {
  pagada: ["Pagada", "bg-exito-fondo text-exito"],
  fiada: ["Fiada", "bg-acento/12 text-acento"],
  anulada: ["Anulada", "bg-peligro/10 text-peligro"],
  conDevolucion: ["Con devolución", "bg-aviso-fondo text-aviso"],
  devuelta: ["Devuelta", "bg-linea text-tenue"],
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
