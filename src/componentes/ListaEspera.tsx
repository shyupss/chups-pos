import { useState } from "react";
import { textoError, type ResumenEspera } from "../api";
import { hora, pesos } from "../formato";
import Dialogo from "./Dialogo";

interface Props {
  /** null mientras se carga. */
  ventas: ResumenEspera[] | null;
  maximo: number;
  /** Hay productos en la caja: al recuperar, esa venta pasa a espera. */
  intercambia: boolean;
  onRecuperar: (id: number) => Promise<void>;
  onDescartar: (id: number) => Promise<void>;
  onCerrar: () => void;
}

const claseBoton = "h-9 rounded-lg px-3 text-tenue hover:bg-fondo hover:text-texto";

export default function ListaEspera({ ventas, maximo, intercambia, onRecuperar, onDescartar, onCerrar }: Props) {
  const [descartando, setDescartando] = useState<number | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const ejecutar = async (accion: () => Promise<void>) => {
    if (ocupado) return;
    setOcupado(true);
    setError(null);
    try {
      await accion();
    } catch (e) {
      setError(textoError(e));
    } finally {
      setOcupado(false);
      setDescartando(null);
    }
  };

  return (
    <Dialogo titulo="Ventas en espera" onCerrar={onCerrar} ancho="w-[34rem]">
      {intercambia && (ventas?.length ?? 0) > 0 && (
        <p className="mt-3 rounded-lg bg-aviso-fondo px-3 py-2 text-aviso">
          La venta que está en la caja quedará en espera al recuperar otra.
        </p>
      )}
      {error && <p className="mt-3 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}

      {ventas === null ? (
        <p className="py-8 text-center text-tenue">Cargando…</p>
      ) : ventas.length === 0 ? (
        <p className="py-8 text-center text-tenue">No hay ventas en espera.</p>
      ) : (
        <ul className="mt-4 max-h-[60vh] overflow-y-auto">
          {ventas.map((v, i) => (
            <li key={v.id} className="flex items-center gap-3 border-t border-linea py-2">
              <div className="min-w-0 flex-1">
                <div className={`truncate font-medium ${v.nombre ? "" : "text-tenue"}`}>{v.nombre ?? "Sin nombre"}</div>
                <div className="text-xs text-tenue tabular-nums">
                  {hora(v.creadaEn)} · {v.lineas} {v.lineas === 1 ? "producto" : "productos"}
                </div>
              </div>
              <div className="shrink-0 font-semibold tabular-nums">{pesos(v.totalEstimado)}</div>
              {descartando === v.id ? (
                <>
                  <button
                    type="button"
                    autoFocus
                    disabled={ocupado}
                    onClick={() => void ejecutar(() => onDescartar(v.id))}
                    className="h-9 rounded-lg bg-peligro/10 px-3 font-medium text-peligro"
                  >
                    Descartar
                  </button>
                  <button type="button" onClick={() => setDescartando(null)} className={claseBoton}>
                    No
                  </button>
                </>
              ) : (
                <>
                  <button
                    type="button"
                    autoFocus={i === 0}
                    disabled={ocupado}
                    onClick={() => void ejecutar(() => onRecuperar(v.id))}
                    className="h-9 rounded-lg bg-acento px-3 font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
                  >
                    Recuperar
                  </button>
                  <button
                    type="button"
                    onClick={() => setDescartando(v.id)}
                    className="h-9 rounded-lg px-3 text-tenue hover:bg-peligro/10 hover:text-peligro"
                  >
                    Descartar
                  </button>
                </>
              )}
            </li>
          ))}
        </ul>
      )}

      <div className="mt-4 flex items-center justify-between border-t border-linea pt-4">
        <span className="text-xs text-tenue tabular-nums">
          {ventas?.length ?? 0} de {maximo} · el total usa los precios actuales
        </span>
        <button type="button" onClick={onCerrar} className={claseBoton}>
          Cerrar <span className="text-xs opacity-60">Esc</span>
        </button>
      </div>
    </Dialogo>
  );
}
