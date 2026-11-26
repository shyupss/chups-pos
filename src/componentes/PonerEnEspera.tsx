import Boton from "./Boton";
import { useState } from "react";
import { textoError } from "../api";
import { pesos } from "../formato";
import Dialogo from "./Dialogo";

interface Props {
  productos: number;
  total: number;
  /** Guarda la venta; si falla, el error se muestra aquí. */
  onConfirmar: (nombre: string | null) => Promise<void>;
  onCancelar: () => void;
}

export default function PonerEnEspera({ productos, total, onConfirmar, onCancelar }: Props) {
  const [nombre, setNombre] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);

  const confirmar = async () => {
    if (enviando) return;
    setEnviando(true);
    setError(null);
    try {
      await onConfirmar(nombre.trim() || null);
    } catch (e) {
      setError(textoError(e));
      setEnviando(false);
    }
  };

  return (
    <Dialogo titulo="Poner en espera" onCerrar={onCancelar}>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void confirmar();
        }}
      >
        <p className="mt-1 text-tenue tabular-nums">
          {productos} {productos === 1 ? "producto" : "productos"} · {pesos(total)}
        </p>
        <label className="mt-4 block">
          <span className="text-sm text-tenue">Nombre para reconocerla (opcional)</span>
          <input
            autoFocus
            value={nombre}
            onChange={(e) => setNombre(e.target.value)}
            placeholder="Señora del pan"
            maxLength={60}
            className="mt-1 h-11 w-full rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento"
            autoComplete="off"
          />
        </label>
        {error && <p className="mt-3 rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}
        <div className="mt-5 flex gap-2">
          <Boton variante="terciario" atajo="Esc" onClick={onCancelar} className="flex-1">
            Cancelar
          </Boton>
          <Boton type="submit" variante="primario" atajo="Enter" disabled={enviando} className="flex-[2]">
            Poner en espera
          </Boton>
        </div>
      </form>
    </Dialogo>
  );
}
