import Boton from "./Boton";
import { useEffect, useState } from "react";
import { api, textoError, type Diferencia, type Unidad } from "../api";
import { cantidad, pesos, pesosConSigno } from "../formato";
import CampoCantidad from "./CampoCantidad";
import Dialogo from "./Dialogo";
import { esCancelado, useConPin } from "./PedirPin";

interface Props {
  producto: { id: number; nombre: string; unidad: Unidad; precioVenta: number; stockMil: number };
  onAjustado: (d: Diferencia) => void;
  onCerrar: () => void;
}

/** Ajuste por conteo de un producto: el stock real que se contó; la diferencia la calcula Rust. */
export default function DialogoAjuste({ producto, onAjustado, onCerrar }: Props) {
  const conPin = useConPin();
  const [contado, setContado] = useState<number | null>(null);
  const [motivo, setMotivo] = useState("Conteo");
  const [vista, setVista] = useState<Diferencia | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);

  // La diferencia contra el stock del momento, calculada en Rust.
  useEffect(() => {
    if (contado === null) return setVista(null);
    const t = setTimeout(() => {
      api
        .previsualizarConteo([{ productoId: producto.id, contadoMil: contado }])
        .then((d) => {
          setVista(d[0]);
          setError(null);
        })
        .catch((e) => {
          setVista(null);
          setError(textoError(e));
        });
    }, 120);
    return () => clearTimeout(t);
  }, [contado, producto.id]);

  const confirmar = async () => {
    if (contado === null || enviando || !motivo.trim()) return;
    setEnviando(true);
    setError(null);
    try {
      const [d] = await conPin(
        (pin) => api.ajustarConteo([{ productoId: producto.id, contadoMil: contado }], motivo, pin),
        { accion: `Ajustar el stock de ${producto.nombre}` },
      );
      onAjustado(d);
    } catch (e) {
      if (!esCancelado(e)) setError(textoError(e));
      setEnviando(false);
    }
  };

  return (
    <Dialogo titulo={`Ajustar stock · ${producto.nombre}`} onCerrar={onCerrar} ancho="w-[28rem]">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void confirmar();
        }}
        className="mt-4 space-y-4"
      >
        <div className="flex items-baseline justify-between tabular-nums">
          <span className="text-tenue">Stock en el sistema</span>
          <span className="text-lg">{cantidad(producto.stockMil, producto.unidad)}</span>
        </div>
        <label className="block">
          <span className="text-tenue">Stock real contado</span>
          <div className="mt-1">
            <CampoCantidad
              producto={producto}
              valorMil={contado}
              onCambiar={setContado}
              minimoMil={0}
              autoFocus
              etiqueta="Stock real contado"
            />
          </div>
        </label>
        {vista && <LineaDiferencia d={vista} />}
        <label className="block">
          <span className="text-tenue">Motivo (obligatorio)</span>
          <input
            value={motivo}
            onChange={(e) => setMotivo(e.target.value)}
            maxLength={200}
            className="mt-1 h-10 w-full rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento"
          />
        </label>
        {error && <p className="rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}
        <div className="flex gap-2">
          <Boton variante="terciario" atajo="Esc" onClick={onCerrar} className="flex-1">
            Cancelar
          </Boton>
          <Boton type="submit" variante="primario" atajo="Enter" disabled={contado === null || enviando || !motivo.trim() || vista?.diferenciaMil === 0} className="flex-[2]">
            {vista?.diferenciaMil === 0 ? "Sin diferencia" : "Ajustar stock"}
          </Boton>
        </div>
      </form>
    </Dialogo>
  );
}

/** "Sobran 2" en aviso o "Faltan 0,182 kg" en rojo, con su valor a costo. */
export function LineaDiferencia({ d }: { d: Diferencia }) {
  const [clase, texto] =
    d.diferenciaMil === 0
      ? ["bg-exito-fondo text-exito", "Coincide con el sistema"]
      : d.diferenciaMil > 0
        ? ["bg-aviso-fondo text-aviso", `Sobran ${cantidad(d.diferenciaMil, d.unidad)}`]
        : ["bg-peligro-fondo text-peligro", `Faltan ${cantidad(-d.diferenciaMil, d.unidad)}`];
  return (
    <div className={`flex items-baseline justify-between rounded-lg px-3 py-2 tabular-nums ${clase}`}>
      <span className="font-medium">{texto}</span>
      <span>{d.valor === null ? "sin costo" : d.valor === 0 ? pesos(0) : pesosConSigno(d.valor)}</span>
    </div>
  );
}
