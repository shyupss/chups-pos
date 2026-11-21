import { useEffect, useRef, useState } from "react";
import { api, textoError, type MedioPago, type NuevaVenta, type VentaConfirmada } from "../api";
import { aEntero, pesos } from "../formato";

interface Props {
  items: NuevaVenta["items"];
  totalEstimado: number;
  onCancelar: () => void;
  onConfirmada: (venta: VentaConfirmada) => void;
}

// Fiado llega en la fase 3, junto con la ficha de clientes.
const MEDIOS: { id: MedioPago; nombre: string; tecla: string }[] = [
  { id: "efectivo", nombre: "Efectivo", tecla: "1" },
  { id: "debito", nombre: "Débito", tecla: "2" },
  { id: "credito", nombre: "Crédito", tecla: "3" },
  { id: "transferencia", nombre: "Transferencia", tecla: "4" },
];

const BILLETES = [1000, 2000, 5000, 10000, 20000];

export default function Cobro({ items, totalEstimado, onCancelar, onConfirmada }: Props) {
  const [medio, setMedio] = useState<MedioPago>("efectivo");
  const [recibido, setRecibido] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);
  const campoRecibido = useRef<HTMLInputElement>(null);

  const montoRecibido = medio === "efectivo" ? aEntero(recibido) : null;
  const vuelto = montoRecibido !== null ? montoRecibido - totalEstimado : 0;
  const falta = medio === "efectivo" && montoRecibido !== null && montoRecibido < totalEstimado;

  useEffect(() => {
    if (medio === "efectivo") campoRecibido.current?.focus();
  }, [medio]);

  const confirmar = async () => {
    if (enviando || falta) return;
    setEnviando(true);
    setError(null);
    try {
      const venta = await api.confirmarVenta({
        items,
        medioPago: medio,
        // Vacío en efectivo = pagó justo.
        montoRecibido: medio === "efectivo" ? (montoRecibido ?? totalEstimado) : null,
        descuento: 0,
        clienteId: null,
      });
      onConfirmada(venta);
    } catch (e) {
      setError(textoError(e));
      setEnviando(false);
    }
  };

  useEffect(() => {
    const alPresionar = (e: KeyboardEvent) => {
      if (e.key === "Escape") return onCancelar();
      if (e.key === "Enter" || e.key === "F12") {
        e.preventDefault();
        return void confirmar();
      }
      const m = MEDIOS.find((x) => x.tecla === e.key);
      if (m && !(e.target instanceof HTMLInputElement)) setMedio(m.id);
      if (m && e.target instanceof HTMLInputElement && e.altKey) setMedio(m.id);
    };
    window.addEventListener("keydown", alPresionar);
    return () => window.removeEventListener("keydown", alPresionar);
  });

  return (
    <div className="fixed inset-0 z-20 flex items-center justify-center bg-slate-900/40">
      <div className="w-[28rem] rounded-xl bg-white p-6 shadow-xl">
        <div className="flex items-baseline justify-between">
          <h2 className="text-lg font-semibold">Cobrar</h2>
          <span className="text-3xl font-semibold tabular-nums">{pesos(totalEstimado)}</span>
        </div>

        <div className="mt-4 grid grid-cols-4 gap-2">
          {MEDIOS.map((m) => (
            <button
              key={m.id}
              onClick={() => setMedio(m.id)}
              className={`rounded-lg border px-2 py-3 text-sm ${
                medio === m.id
                  ? "border-slate-900 bg-slate-900 text-white"
                  : "border-slate-200 hover:bg-slate-50"
              }`}
            >
              {m.nombre}
              <div className="text-xs opacity-60">{medio === "efectivo" ? `Alt+${m.tecla}` : m.tecla}</div>
            </button>
          ))}
        </div>

        {medio === "efectivo" && (
          <div className="mt-5">
            <label className="text-sm text-slate-500">Recibido (vacío = justo)</label>
            <input
              ref={campoRecibido}
              value={recibido ? pesos(aEntero(recibido) ?? 0) : ""}
              onChange={(e) => setRecibido(e.target.value)}
              className="mt-1 w-full rounded-lg border border-slate-300 px-4 py-3 text-2xl tabular-nums outline-none focus:border-slate-900"
              inputMode="numeric"
              placeholder={pesos(totalEstimado)}
            />
            <div className="mt-2 flex gap-2">
              {BILLETES.filter((b) => b >= totalEstimado || b === 20000).map((b) => (
                <button
                  key={b}
                  onClick={() => setRecibido(String(b))}
                  className="rounded border border-slate-200 px-2 py-1 text-sm hover:bg-slate-50"
                >
                  {pesos(b)}
                </button>
              ))}
            </div>
            <div className="mt-4 flex items-baseline justify-between">
              <span className="text-slate-500">Vuelto</span>
              <span className={`text-3xl font-semibold tabular-nums ${falta ? "text-red-700" : ""}`}>
                {falta ? `Faltan ${pesos(-vuelto)}` : pesos(Math.max(0, vuelto))}
              </span>
            </div>
          </div>
        )}

        {error && <p className="mt-4 rounded bg-red-50 px-3 py-2 text-sm text-red-800">{error}</p>}

        <div className="mt-6 flex gap-2">
          <button onClick={onCancelar} className="flex-1 rounded-lg py-3 text-slate-600 hover:bg-slate-100">
            Volver <span className="text-xs opacity-60">Esc</span>
          </button>
          <button
            onClick={() => void confirmar()}
            disabled={enviando || falta}
            className="flex-[2] rounded-lg bg-emerald-700 py-3 font-semibold text-white disabled:bg-slate-300"
          >
            Confirmar venta <span className="text-xs font-normal opacity-70">Enter</span>
          </button>
        </div>
      </div>
    </div>
  );
}
