import { LockOpen } from "lucide-react";
import { useState } from "react";
import { api, textoError, type Turno } from "../api";
import { aEntero, pesos } from "../formato";

const BILLETES = [1000, 2000, 5000, 10000, 20000];

interface Props {
  /** Monto inicial del último turno. */
  sugerido: number | null;
  /** Ventas en espera guardadas: se recuperan al abrir. */
  enEspera: number;
  onAbierta: (turno: Turno) => void;
}

/** Lo que muestra la Caja mientras no hay turno: el monto con que se empieza. Enter abre. */
export default function AbrirCaja({ sugerido, enEspera, onAbierta }: Props) {
  const [texto, setTexto] = useState(sugerido !== null ? String(sugerido) : "");
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);
  const monto = aEntero(texto) ?? 0;

  const abrir = async () => {
    if (enviando) return;
    setEnviando(true);
    setError(null);
    try {
      onAbierta(await api.abrirTurno(monto));
    } catch (e) {
      setError(textoError(e));
      setEnviando(false);
    }
  };

  return (
    <div className="flex h-full items-center justify-center p-6">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void abrir();
        }}
        className="w-[30rem] max-w-full rounded-lg border border-linea bg-superficie p-8"
      >
        <div className="flex items-center gap-3">
          <span className="flex size-10 items-center justify-center rounded-lg bg-acento/12 text-acento">
            <LockOpen size={20} aria-hidden />
          </span>
          <div>
            <h1 className="text-xl font-semibold">Abrir caja</h1>
            <p className="text-tenue">Cuenta el efectivo con que empiezas el turno.</p>
          </div>
        </div>

        <label className="mt-6 block">
          <span className="text-tenue">Monto inicial</span>
          <input
            autoFocus
            onFocus={(e) => e.currentTarget.select()}
            value={texto ? pesos(monto) : ""}
            onChange={(e) => setTexto(e.target.value)}
            placeholder="$0"
            inputMode="numeric"
            aria-describedby="sugerido-caja"
            className="mt-1 h-14 w-full rounded-lg border border-linea bg-fondo px-4 text-3xl font-semibold tabular-nums outline-none focus:border-texto"
          />
        </label>
        <div className="mt-2 flex flex-wrap gap-2">
          {BILLETES.map((b) => (
            <button
              key={b}
              type="button"
              onClick={() => setTexto(String(monto + b))}
              className="h-8 rounded-lg border border-linea px-2 tabular-nums hover:bg-fondo"
              title={`Sumar un billete de ${pesos(b)}`}
            >
              +{pesos(b)}
            </button>
          ))}
          <button
            type="button"
            onClick={() => setTexto("")}
            className="h-8 rounded-lg px-2 text-tenue hover:bg-fondo hover:text-texto"
          >
            Limpiar
          </button>
        </div>
        <p id="sugerido-caja" className="mt-3 text-xs text-tenue">
          {sugerido !== null
            ? `Sugerido: ${pesos(sugerido)}, el monto inicial del turno anterior.`
            : "Es el primer turno: escribe el efectivo que hay en el cajón (puede ser $0)."}
          {enEspera > 0 &&
            ` Hay ${enEspera === 1 ? "1 venta en espera" : `${enEspera} ventas en espera`} para recuperar después de abrir.`}
        </p>

        {error && <p className="mt-4 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}

        <button
          type="submit"
          disabled={enviando}
          className="mt-6 h-13 w-full rounded-lg bg-acento text-lg font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
        >
          Abrir caja con {pesos(monto)} <span className="text-sm font-normal opacity-70">Enter</span>
        </button>
      </form>
    </div>
  );
}
