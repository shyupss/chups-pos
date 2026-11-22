import Boton from "./Boton";
import { ArrowDownToLine, ArrowUpFromLine } from "lucide-react";
import { useEffect, useState } from "react";
import { api, textoError, type MovimientoCaja, type TipoMovimiento } from "../api";
import { aEntero, pesos } from "../formato";
import Dialogo from "./Dialogo";
import Opciones, { type Opcion } from "./Opciones";
import { esCancelado, useConPin } from "./PedirPin";

const TIPOS: Opcion<TipoMovimiento>[] = [
  { id: "ingreso", nombre: "Ingreso", Icono: ArrowDownToLine },
  { id: "retiro", nombre: "Retiro", Icono: ArrowUpFromLine },
];

interface Props {
  turnoId: number;
  onRegistrado: (m: MovimientoCaja) => void;
  onCerrar: () => void;
}

/** Ingreso (sencillo, cambio) o retiro (pago a un proveedor, depósito) de efectivo con su motivo. */
export default function DialogoMovimiento({ turnoId, onRegistrado, onCerrar }: Props) {
  const conPin = useConPin();
  const [tipo, setTipo] = useState<TipoMovimiento>("ingreso");
  const [monto, setMonto] = useState("");
  const [motivo, setMotivo] = useState("");
  const [esperado, setEsperado] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);
  const valor = aEntero(monto) ?? 0;
  const listo = valor > 0 && motivo.trim() !== "";

  useEffect(() => {
    api
      .resumenTurno(turnoId)
      .then((r) => setEsperado(r.efectivo.esperado))
      .catch(() => {});
  }, [turnoId]);

  const registrar = async () => {
    if (!listo || enviando) return;
    setEnviando(true);
    setError(null);
    try {
      const m = await conPin((pin) => api.registrarMovimientoCaja(tipo, valor, motivo, pin), {
        accion: `Retirar ${pesos(valor)} de la caja`,
      });
      onRegistrado(m);
    } catch (e) {
      if (!esCancelado(e)) setError(textoError(e));
      setEnviando(false);
    }
  };

  return (
    <Dialogo titulo="Ingreso o retiro de efectivo" onCerrar={onCerrar} ancho="w-[28rem]">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void registrar();
        }}
        className="mt-4 space-y-4"
      >
        <Opciones nombre="tipo-movimiento" etiqueta="Tipo de movimiento" opciones={TIPOS} valor={tipo} onCambiar={setTipo} />
        <label className="block">
          <span className="text-tenue">Monto</span>
          <input
            autoFocus
            value={monto ? pesos(valor) : ""}
            onChange={(e) => setMonto(e.target.value)}
            placeholder="$0"
            inputMode="numeric"
            className="mt-1 h-13 w-full rounded-lg border border-borde-control bg-superficie px-4 text-2xl tabular-nums focus:border-acento"
          />
        </label>
        <label className="block">
          <span className="text-tenue">Motivo (obligatorio)</span>
          <input
            value={motivo}
            onChange={(e) => setMotivo(e.target.value)}
            maxLength={200}
            placeholder={tipo === "ingreso" ? "Sencillo del banco" : "Pago al proveedor de pan"}
            className="mt-1 h-10 w-full rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento"
          />
        </label>
        {esperado !== null && (
          <p className="text-tenue tabular-nums">
            En la caja debería haber {pesos(esperado)}
            {valor > 0 && ` · quedarían ${pesos(tipo === "ingreso" ? esperado + valor : esperado - valor)}`}
          </p>
        )}
        {error && <p className="rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}
        <div className="flex gap-2">
          <Boton variante="terciario" atajo="Esc" onClick={onCerrar} className="flex-1">
            Cancelar
          </Boton>
          <Boton type="submit" variante="primario" atajo="Enter" disabled={!listo || enviando} className="flex-[2]">
            Registrar {tipo}
          </Boton>
        </div>
      </form>
    </Dialogo>
  );
}
