import { useState } from "react";
import { api, textoError, type DetalleVenta, type MedioPago } from "../api";
import { folio, NOMBRE_MEDIO, pesos, redondeoEfectivo } from "../formato";
import Dialogo from "./Dialogo";
import Opciones, { type Opcion } from "./Opciones";
import { esCancelado, useConPin } from "./PedirPin";

/** Medios en que se puede devolver el dinero (no se devuelve "fiado"). */
export const MEDIOS_REEMBOLSO: Opcion<Exclude<MedioPago, "fiado">>[] = [
  { id: "efectivo", nombre: "Efectivo" },
  { id: "debito", nombre: "Débito" },
  { id: "credito", nombre: "Crédito" },
  { id: "transferencia", nombre: "Transferencia" },
];

interface Props {
  venta: DetalleVenta;
  /** Si el efectivo se redondea a la decena (para mostrar el monto antes de confirmar). */
  redondeoActivo: boolean;
  onAnulada: (venta: DetalleVenta) => void;
  onCerrar: () => void;
}

/** Anular toda la venta: repone el stock y devuelve el dinero (o anula la deuda si fue fiada). */
export default function DialogoAnular({ venta, redondeoActivo, onAnulada, onCerrar }: Props) {
  const conPin = useConPin();
  const fiada = venta.medioPago === "fiado";
  const [medio, setMedio] = useState<Exclude<MedioPago, "fiado">>(
    venta.medioPago === "fiado" ? "efectivo" : venta.medioPago,
  );
  const [motivo, setMotivo] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);

  // Solo para mostrar: el monto que vale lo calcula Rust con la misma regla.
  const monto = medio === "efectivo" && redondeoActivo ? venta.total + redondeoEfectivo(venta.total) : venta.total;

  const anular = async () => {
    if (enviando || !motivo.trim()) return;
    setEnviando(true);
    setError(null);
    try {
      const d = await conPin((pin) => api.anularVenta(venta.id, motivo, fiada && venta.abonado === 0 ? null : medio, pin), {
        accion: `Anular la venta ${folio(venta.folio)}`,
      });
      onAnulada(d);
    } catch (e) {
      if (!esCancelado(e)) setError(textoError(e));
      setEnviando(false);
    }
  };

  return (
    <Dialogo titulo={`Anular venta ${folio(venta.folio)}`} onCerrar={onCerrar} ancho="w-[30rem]">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void anular();
        }}
        className="mt-3 space-y-4"
      >
        <p className="text-tenue">
          Todos los productos vuelven al stock y la venta deja de contar. La venta queda guardada como anulada: no se
          borra.
        </p>
        {fiada ? (
          <>
            <p className="rounded-lg bg-acento/12 px-3 py-2 text-acento">
              Venta fiada{venta.cliente ? ` de ${venta.cliente}` : ""}: deja de contar como deuda.
              {venta.abonado > 0 &&
                ` Lo que ya abonó (${pesos(venta.abonado)}) pasa como saldo a favor a sus otras deudas; si no tiene, se le devuelve.`}
            </p>
            {venta.abonado > 0 && <div>
            <span className="text-tenue">Si sobra, devolver en</span>
            <div className="mt-1">
              <Opciones nombre="medio-reembolso" etiqueta="Medio de reembolso" opciones={MEDIOS_REEMBOLSO} valor={medio} onCambiar={setMedio} />
            </div>
          </div>}
          </>
        ) : (
          <div>
            <span className="text-tenue">Devolver el dinero en</span>
            <div className="mt-1">
              <Opciones nombre="medio-reembolso" etiqueta="Medio de reembolso" opciones={MEDIOS_REEMBOLSO} valor={medio} onCambiar={setMedio} />
            </div>
            <p className="mt-1 text-xs text-tenue">
              Se pagó con {NOMBRE_MEDIO[venta.medioPago].toLowerCase()}.
              {medio === "efectivo" && " El efectivo sale de la caja abierta."}
            </p>
          </div>
        )}
        <div className="flex items-baseline justify-between tabular-nums">
          <span className="text-tenue">{fiada ? "Deuda que se anula" : "A devolver"}</span>
          <span className="text-3xl font-semibold">{pesos(fiada ? (venta.saldo ?? 0) : monto)}</span>
        </div>
        <label className="block">
          <span className="text-tenue">Motivo (obligatorio)</span>
          <input
            autoFocus
            value={motivo}
            onChange={(e) => setMotivo(e.target.value)}
            maxLength={200}
            placeholder="Por ejemplo, se cobró dos veces"
            className="mt-1 h-10 w-full rounded-lg border border-linea bg-fondo px-3 outline-none focus:border-texto"
          />
        </label>
        {error && <p className="rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}
        <div className="flex gap-2">
          <button type="button" onClick={onCerrar} className="h-11 flex-1 rounded-lg text-tenue hover:bg-fondo hover:text-texto">
            Volver <span className="text-xs opacity-60">Esc</span>
          </button>
          <button
            type="submit"
            disabled={enviando || !motivo.trim()}
            className="h-11 flex-[2] rounded-lg bg-peligro font-semibold text-superficie disabled:bg-linea disabled:text-tenue"
          >
            Anular venta <span className="text-xs font-normal opacity-70">Enter</span>
          </button>
        </div>
      </form>
    </Dialogo>
  );
}
