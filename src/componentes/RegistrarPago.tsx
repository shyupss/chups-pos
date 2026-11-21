import { useState } from "react";
import { api, textoError, type Cliente, type MedioPago, type PagoRegistrado, type VentaFiada } from "../api";
import { aEntero, folio, pesos, redondeoEfectivo } from "../formato";
import Dialogo from "./Dialogo";
import { MEDIOS_REEMBOLSO } from "./DialogoAnular";
import Opciones from "./Opciones";

interface Props {
  cliente: Cliente;
  /** Ventas marcadas en la ficha: el pago va primero a ellas. */
  marcadas: VentaFiada[];
  /** Si el efectivo se redondea a la decena (config redondeo_efectivo). */
  redondeoActivo: boolean;
  onRegistrado: (p: PagoRegistrado) => void;
  onCerrar: () => void;
}

const claseCampo =
  "h-12 w-full rounded-lg border border-linea bg-fondo px-4 text-2xl tabular-nums outline-none focus:border-texto";

/** Registrar lo que entrega el cliente: parcial, lo marcado o "Paga todo". Nunca más que la deuda. */
export default function RegistrarPago({ cliente, marcadas, redondeoActivo, onRegistrado, onCerrar }: Props) {
  const deMarcadas = marcadas.reduce((s, v) => s + v.saldo, 0);
  const [monto, setMonto] = useState(marcadas.length > 0 ? String(deMarcadas) : "");
  const [medio, setMedio] = useState<Exclude<MedioPago, "fiado">>("efectivo");
  const [recibido, setRecibido] = useState("");
  const [nota, setNota] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);

  const valor = aEntero(monto) ?? 0;
  const efectivo = medio === "efectivo";
  // Solo para mostrar: el redondeo y el vuelto que valen los calcula Rust con la misma regla.
  const redondeo = efectivo && redondeoActivo && valor > 0 ? redondeoEfectivo(valor) : 0;
  const entra = valor + redondeo;
  const valorRecibido = efectivo ? aEntero(recibido) : null;
  const vuelto = valorRecibido !== null ? valorRecibido - entra : 0;
  const pasado = valor > cliente.deuda;
  const falta = efectivo && valorRecibido !== null && valorRecibido < entra;
  const bloqueado = enviando || valor <= 0 || pasado || falta;

  const registrar = async () => {
    if (bloqueado) return;
    setEnviando(true);
    setError(null);
    try {
      onRegistrado(
        await api.registrarPago({
          clienteId: cliente.id,
          monto: valor,
          medioPago: medio,
          ventas: marcadas.map((v) => v.ventaId),
          montoRecibido: efectivo ? valorRecibido : null,
          nota: nota.trim() || null,
        }),
      );
    } catch (e) {
      setError(textoError(e));
      setEnviando(false);
    }
  };

  return (
    <Dialogo titulo={`Registrar pago de ${cliente.nombre}`} onCerrar={onCerrar} ancho="w-[32rem]">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void registrar();
        }}
        className="mt-3 space-y-4"
      >
        <div className="flex items-baseline justify-between tabular-nums">
          <span className="text-tenue">Debe</span>
          <span className="text-2xl font-semibold">{pesos(cliente.deuda)}</span>
        </div>

        <label className="block">
          <span className="text-tenue">Monto del pago</span>
          <input
            autoFocus
            value={monto ? pesos(valor) : ""}
            onChange={(e) => setMonto(e.target.value)}
            inputMode="numeric"
            placeholder="$0"
            aria-invalid={pasado}
            className={`mt-1 ${claseCampo}`}
          />
        </label>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => setMonto(String(cliente.deuda))}
            className="h-8 rounded-lg border border-linea px-3 hover:bg-fondo"
          >
            Paga todo ({pesos(cliente.deuda)})
          </button>
          {marcadas.length > 0 && (
            <button
              type="button"
              onClick={() => setMonto(String(deMarcadas))}
              className="h-8 rounded-lg border border-linea px-3 hover:bg-fondo"
            >
              Lo marcado ({pesos(deMarcadas)})
            </button>
          )}
        </div>
        <p className="text-xs text-tenue">
          {marcadas.length > 0
            ? `Va primero a ${marcadas.map((v) => folio(v.folio)).join(", ")}; lo que sobre, a las más antiguas.`
            : "Se aplica de la venta más antigua a la más nueva."}
        </p>

        <div>
          <span className="text-tenue">Medio</span>
          <div className="mt-1">
            <Opciones nombre="medio-pago" etiqueta="Medio del pago" opciones={MEDIOS_REEMBOLSO} valor={medio} onCambiar={setMedio} />
          </div>
        </div>

        {efectivo && valor > 0 && (
          <div className="space-y-2 rounded-lg border border-linea p-3 tabular-nums">
            {redondeo !== 0 && (
              <div className="flex justify-between text-tenue">
                <span>Redondeo</span>
                <span>
                  {redondeo > 0 ? "+" : "−"}
                  {pesos(Math.abs(redondeo))} · paga {pesos(entra)}
                </span>
              </div>
            )}
            <label className="flex items-center justify-between gap-3">
              <span className="text-tenue">Recibido (vacío = justo)</span>
              <input
                value={recibido ? pesos(aEntero(recibido) ?? 0) : ""}
                onChange={(e) => setRecibido(e.target.value)}
                inputMode="numeric"
                placeholder={pesos(entra)}
                className="h-10 w-40 rounded-lg border border-linea bg-fondo px-3 text-right outline-none focus:border-texto"
              />
            </label>
            <div className="flex items-baseline justify-between">
              <span className="text-tenue">Vuelto</span>
              <span className={`text-xl font-semibold ${falta ? "text-peligro" : ""}`}>
                {falta ? `Faltan ${pesos(-vuelto)}` : pesos(Math.max(0, vuelto))}
              </span>
            </div>
          </div>
        )}

        <label className="block">
          <span className="text-tenue">Nota (opcional)</span>
          <input
            value={nota}
            onChange={(e) => setNota(e.target.value)}
            maxLength={200}
            className="mt-1 h-10 w-full rounded-lg border border-linea bg-fondo px-3 outline-none focus:border-texto"
          />
        </label>

        <div className="flex items-baseline justify-between border-t border-linea pt-3 tabular-nums">
          <span className="text-tenue">Queda debiendo</span>
          <span className={`text-xl font-semibold ${pasado ? "text-peligro" : ""}`}>
            {pasado ? `El pago supera la deuda en ${pesos(valor - cliente.deuda)}` : pesos(cliente.deuda - valor)}
          </span>
        </div>
        {error && <p className="rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}

        <div className="flex gap-2">
          <button type="button" onClick={onCerrar} className="h-11 flex-1 rounded-lg text-tenue hover:bg-fondo hover:text-texto">
            Cancelar <span className="text-xs opacity-60">Esc</span>
          </button>
          <button
            type="submit"
            disabled={bloqueado}
            className="h-11 flex-[2] rounded-lg bg-acento font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
          >
            Registrar pago <span className="text-xs font-normal opacity-70">Enter</span>
          </button>
        </div>
      </form>
    </Dialogo>
  );
}
