import Boton from "./Boton";
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
  /** Sin licencia después de la prueba no se abre la caja (lo decide Rust; aquí solo se avisa). */
  bloqueada?: boolean;
  onIrALicencia?: () => void;
}

/** Lo que muestra la Caja mientras no hay turno: el monto con que se empieza. Enter abre. */
export default function AbrirCaja({ sugerido, enEspera, onAbierta, bloqueada, onIrALicencia }: Props) {
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
        className="w-[32rem] max-w-full tarjeta p-8"
      >
        <div className="flex items-center gap-3">
          <span className="flex size-10 items-center justify-center rounded-lg bg-seleccion text-acento">
            <LockOpen size={20} aria-hidden />
          </span>
          <div>
            <h1 className="text-xl font-semibold">Abrir caja</h1>
            <p className="text-tenue">Cuenta el efectivo con que empiezas el turno.</p>
          </div>
        </div>

        <div className="mt-6 flex items-baseline justify-between">
          <label htmlFor="monto-inicial" className="text-tenue">
            Monto inicial
          </label>
          <Boton variante="terciario" tamano="chico" onClick={() => setTexto("")}>
            Limpiar
          </Boton>
        </div>
        <input
          id="monto-inicial"
          autoFocus
          onFocus={(e) => e.currentTarget.select()}
          value={texto ? pesos(monto) : ""}
          onChange={(e) => setTexto(e.target.value)}
          placeholder="$0"
          inputMode="numeric"
          aria-describedby="sugerido-caja"
          className="mt-1 h-14 w-full rounded-lg border border-borde-control bg-superficie px-4 text-3xl font-semibold tabular-nums focus:border-acento"
        />
        <div className="mt-2 flex flex-wrap gap-2">
          {BILLETES.map((b) => (
            <Boton
              key={b}
              tamano="chico"
              onClick={() => setTexto(String(monto + b))}
              className="tabular-nums"
              title={`Sumar un billete de ${pesos(b)}`}
            >
              +{pesos(b)}
            </Boton>
          ))}
        </div>
        <p id="sugerido-caja" className="mt-3 text-xs text-tenue">
          {sugerido !== null
            ? `Sugerido: ${pesos(sugerido)}, el monto inicial del turno anterior.`
            : "Es el primer turno: escribe el efectivo que hay en el cajón (puede ser $0)."}
          {enEspera > 0 &&
            ` Hay ${enEspera === 1 ? "1 venta en espera" : `${enEspera} ventas en espera`} para recuperar después de abrir.`}
        </p>

        {error && <p className="mt-4 rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}
        {bloqueada && (
          <div className="mt-4 flex flex-wrap items-center gap-3 rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">
            <span className="min-w-0 flex-1">
              La prueba terminó: para abrir la caja activa la licencia. Tus datos siguen disponibles.
            </span>
            {onIrALicencia && (
              <Boton tamano="chico" onClick={onIrALicencia}>
                Activar licencia
              </Boton>
            )}
          </div>
        )}

        <Boton
          type="submit"
          variante="primario"
          atajo="Enter"
          disabled={enviando || bloqueada}
          className="mt-6 h-12 w-full text-base"
        >
          Abrir caja con {pesos(monto)}
        </Boton>
      </form>
    </div>
  );
}
