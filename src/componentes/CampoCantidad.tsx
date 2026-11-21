import { Scale } from "lucide-react";
import { useState } from "react";
import type { Unidad } from "../api";
import { cantidad } from "../formato";
import DialogoPeso from "./DialogoPeso";

interface Props {
  producto: { nombre: string; unidad: Unidad; precioVenta: number };
  /** En milésimas; null mientras no se ha escrito. */
  valorMil: number | null;
  onCambiar: (mil: number | null) => void;
  /** 0 en un conteo (puede no quedar nada); 1 por defecto. */
  minimoMil?: number;
  autoFocus?: boolean;
  etiqueta: string;
  onEnter?: () => void;
}

/** Cantidad de un producto: unidades enteras en un campo, o kg con el diálogo de peso. */
export default function CampoCantidad({ producto, valorMil, onCambiar, minimoMil = 1, autoFocus, etiqueta, onEnter }: Props) {
  const [pesando, setPesando] = useState(false);
  if (producto.unidad === "kg") {
    return (
      <>
        <button
          type="button"
          autoFocus={autoFocus}
          onClick={() => setPesando(true)}
          aria-label={etiqueta}
          className="flex h-11 w-full items-center gap-2 rounded-lg border border-linea bg-fondo px-3 text-left tabular-nums hover:border-texto"
        >
          <Scale size={16} className="text-tenue" aria-hidden />
          {valorMil === null ? <span className="text-tenue">Pesar o escribir el peso…</span> : cantidad(valorMil, "kg")}
        </button>
        {pesando && (
          <DialogoPeso
            producto={producto}
            inicialMil={valorMil ?? undefined}
            minimoMil={minimoMil}
            accion="Listo"
            onConfirmar={(mil) => {
              onCambiar(mil);
              setPesando(false);
              onEnter?.();
            }}
            onCancelar={() => setPesando(false)}
          />
        )}
      </>
    );
  }
  return (
    <input
      autoFocus={autoFocus}
      value={valorMil === null ? "" : String(valorMil / 1000)}
      onChange={(e) => {
        const d = e.target.value.replace(/\D/g, "").slice(0, 9);
        onCambiar(d === "" ? null : Number(d) * 1000);
      }}
      onFocus={(e) => e.currentTarget.select()}
      onKeyDown={(e) => {
        if (e.key === "Enter" && onEnter) {
          e.preventDefault();
          onEnter();
        }
      }}
      inputMode="numeric"
      placeholder="0"
      aria-label={etiqueta}
      className="h-11 w-full rounded-lg border border-borde-control bg-superficie px-3 text-lg tabular-nums focus:border-acento"
    />
  );
}
