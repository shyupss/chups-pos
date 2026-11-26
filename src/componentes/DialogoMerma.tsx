import Boton from "./Boton";
import { useState } from "react";
import { api, textoError, type MotivoMerma, type Unidad } from "../api";
import { cantidad } from "../formato";
import CampoCantidad from "./CampoCantidad";
import Dialogo from "./Dialogo";
import Opciones, { type Opcion } from "./Opciones";
import { esCancelado, useConPin } from "./PedirPin";

const MOTIVOS: Opcion<MotivoMerma>[] = [
  { id: "vencido", nombre: "Vencido" },
  { id: "danado", nombre: "Dañado" },
  { id: "robo", nombre: "Robo" },
  { id: "consumoInterno", nombre: "Consumo interno" },
  { id: "otro", nombre: "Otro" },
];

interface Props {
  producto: { id: number; nombre: string; unidad: Unidad; precioVenta: number; stockMil: number };
  /** Recibe el stock que quedó. */
  onRegistrada: (stockMil: number) => void;
  onCerrar: () => void;
}

/** Saca del stock lo que se perdió, con su motivo (movimiento 'merma'). */
export default function DialogoMerma({ producto, onRegistrada, onCerrar }: Props) {
  const conPin = useConPin();
  const [mil, setMil] = useState<number | null>(null);
  const [motivo, setMotivo] = useState<MotivoMerma>("vencido");
  const [detalle, setDetalle] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);
  const listo = mil !== null && mil > 0 && (motivo !== "otro" || detalle.trim() !== "");

  const registrar = async () => {
    if (!listo || enviando || mil === null) return;
    setEnviando(true);
    setError(null);
    try {
      const stock = await conPin(
        (pin) => api.registrarMerma(producto.id, mil, motivo, detalle.trim() || null, pin),
        { accion: `Registrar una merma de ${producto.nombre}` },
      );
      onRegistrada(stock);
    } catch (e) {
      if (!esCancelado(e)) setError(textoError(e));
      setEnviando(false);
    }
  };

  return (
    <Dialogo titulo={`Merma · ${producto.nombre}`} onCerrar={onCerrar} ancho="w-[32rem]">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void registrar();
        }}
        className="mt-4 space-y-4"
      >
        <div className="flex items-baseline justify-between tabular-nums">
          <span className="text-tenue">Stock actual</span>
          <span>{cantidad(producto.stockMil, producto.unidad)}</span>
        </div>
        <label className="block">
          <span className="text-tenue">Cantidad perdida</span>
          <div className="mt-1">
            <CampoCantidad producto={producto} valorMil={mil} onCambiar={setMil} autoFocus etiqueta="Cantidad perdida" />
          </div>
        </label>
        <div>
          <span className="text-tenue">Motivo</span>
          <div className="mt-1">
            <Opciones nombre="motivo-merma" etiqueta="Motivo de la merma" opciones={MOTIVOS} valor={motivo} onCambiar={setMotivo} />
          </div>
        </div>
        <label className="block">
          <span className="text-tenue">{motivo === "otro" ? "Detalle (obligatorio)" : "Detalle (opcional)"}</span>
          <input
            value={detalle}
            onChange={(e) => setDetalle(e.target.value)}
            maxLength={200}
            placeholder="Por ejemplo, se rompió al reponer"
            className="mt-1 h-11 w-full rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento"
          />
        </label>
        {error && <p className="rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}
        <div className="flex gap-2">
          <Boton variante="terciario" atajo="Esc" onClick={onCerrar} className="flex-1">
            Cancelar
          </Boton>
          <Boton type="submit" variante="primario" atajo="Enter" disabled={!listo || enviando} className="flex-[2]">
            Registrar merma
          </Boton>
        </div>
      </form>
    </Dialogo>
  );
}
