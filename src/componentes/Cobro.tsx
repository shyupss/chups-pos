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
    <div className="fixed inset-0 z-20 flex items-center justify-center bg-velo">
      <div className="w-[28rem] rounded-lg border border-linea bg-superficie p-6 shadow-xl">
        <div className="flex items-baseline justify-between">
          <h2 className="text-lg font-semibold">Cobrar</h2>
          <span className="text-3xl font-semibold tabular-nums">{pesos(totalEstimado)}</span>
        </div>

        <div className="mt-4 grid grid-cols-4 gap-2">
          {MEDIOS.map((m) => (
            <button
              key={m.id}
              onClick={() => setMedio(m.id)}
              className={`rounded-lg border px-2 py-3 ${
                medio === m.id
                  ? "border-texto bg-texto font-medium text-superficie"
                  : "border-linea hover:bg-fondo"
              }`}
            >
              {m.nombre}
              <div className="text-xs opacity-60">{medio === "efectivo" ? `Alt+${m.tecla}` : m.tecla}</div>
            </button>
          ))}
        </div>

        {medio === "efectivo" && (
          <div className="mt-5">
            <label className="text-tenue">Recibido (vacío = justo)</label>
            <input
              ref={campoRecibido}
              value={recibido ? pesos(aEntero(recibido) ?? 0) : ""}
              onChange={(e) => setRecibido(e.target.value)}
              className="mt-1 h-13 w-full rounded-lg border border-linea bg-fondo px-4 text-2xl tabular-nums outline-none focus:border-texto"
              inputMode="numeric"
              placeholder={pesos(totalEstimado)}
            />
            <div className="mt-2 flex gap-2">
              {BILLETES.filter((b) => b >= totalEstimado || b === 20000).map((b) => (
                <button
                  key={b}
                  onClick={() => setRecibido(String(b))}
                  className="h-8 rounded-lg border border-linea px-2 tabular-nums hover:bg-fondo"
                >
                  {pesos(b)}
                </button>
              ))}
            </div>
            <div className="mt-4 flex items-baseline justify-between">
              <span className="text-tenue">Vuelto</span>
              <span className={`text-3xl font-semibold tabular-nums ${falta ? "text-peligro" : ""}`}>
                {falta ? `Faltan ${pesos(-vuelto)}` : pesos(Math.max(0, vuelto))}
              </span>
            </div>
          </div>
        )}

        {error && <p className="mt-4 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}

        <div className="mt-6 flex gap-2">
          <button onClick={onCancelar} className="h-12 flex-1 rounded-lg text-tenue hover:bg-fondo hover:text-texto">
            Volver <span className="text-xs opacity-60">Esc</span>
          </button>
          <button
            onClick={() => void confirmar()}
            disabled={enviando || falta}
            className="h-12 flex-[2] rounded-lg bg-acento font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
          >
            Confirmar venta <span className="text-xs font-normal opacity-70">Enter</span>
          </button>
        </div>
      </div>
    </div>
  );
}
