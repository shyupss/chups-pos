import type { ReactNode } from "react";

/** El color es el del rol: éxito, aviso, peligro, información o neutro (ver "Sistema de color por rol" del README). */
export type TonoInsignia = "exito" | "aviso" | "peligro" | "info" | "neutro";

const TONOS: Record<TonoInsignia, string> = {
  exito: "bg-exito-fondo text-exito",
  aviso: "bg-aviso-fondo text-aviso",
  peligro: "border border-peligro/50 bg-peligro-fondo text-peligro",
  info: "bg-info-fondo text-info",
  neutro: "bg-suave text-tenue",
};

/** Un estado en una tabla o en un detalle: "Bajo el mínimo", "Anulada", "Sin costo"… con fondo, no solo color de texto. */
export default function Insignia({ tono, children }: { tono: TonoInsignia; children: ReactNode }) {
  return (
    <span className={`inline-flex h-6 shrink-0 items-center rounded-full px-2.5 text-xs font-medium whitespace-nowrap ${TONOS[tono]}`}>
      {children}
    </span>
  );
}
