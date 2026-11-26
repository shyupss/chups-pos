import Boton from "../../componentes/Boton";
import { FileDown, FileSpreadsheet } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useAtajos } from "../../atajos";
import { api, textoError, type FormatoExportacion, type TipoReporte } from "../../api";
import { diaLocal } from "../../formato";

export type Atajo = "hoy" | "ayer" | "semana" | "mes" | "mesPasado" | "ultimos30" | "anio" | "personalizado";

export interface Rango {
  atajo: Atajo;
  /** "aaaa-mm-dd" locales, ambos incluidos. */
  desde: string;
  hasta: string;
}

const ATAJOS: { id: Atajo; nombre: string }[] = [
  { id: "hoy", nombre: "Hoy" },
  { id: "ayer", nombre: "Ayer" },
  { id: "semana", nombre: "Esta semana" },
  { id: "mes", nombre: "Este mes" },
  { id: "mesPasado", nombre: "Mes pasado" },
  { id: "ultimos30", nombre: "Últimos 30 días" },
  { id: "anio", nombre: "Este año" },
  { id: "personalizado", nombre: "Personalizado" },
];

/** Las fechas de un atajo, en el calendario local (la semana empieza el lunes). */
export function rangoDe(atajo: Atajo, hoy = new Date()): Rango {
  const d = (a: number, m: number, dia: number) => diaLocal(new Date(a, m, dia));
  const [a, m, dia] = [hoy.getFullYear(), hoy.getMonth(), hoy.getDate()];
  const deHoy = diaLocal(hoy);
  switch (atajo) {
    case "ayer":
      return { atajo, desde: d(a, m, dia - 1), hasta: d(a, m, dia - 1) };
    case "semana":
      return { atajo, desde: d(a, m, dia - ((hoy.getDay() + 6) % 7)), hasta: deHoy };
    case "mes":
      return { atajo, desde: d(a, m, 1), hasta: deHoy };
    case "mesPasado":
      return { atajo, desde: d(a, m - 1, 1), hasta: d(a, m, 0) };
    case "ultimos30":
      return { atajo, desde: d(a, m, dia - 29), hasta: deHoy };
    case "anio":
      return { atajo, desde: d(a, 0, 1), hasta: deHoy };
    default:
      return { atajo, desde: deHoy, hasta: deHoy };
  }
}

export function SelectorRango({ rango, onCambiar }: { rango: Rango; onCambiar: (r: Rango) => void }) {
  return (
    <div className="flex flex-wrap items-center gap-2">
      <div role="radiogroup" aria-label="Rango de fechas" className="flex flex-wrap gap-1 rounded-lg border border-borde-control bg-suave p-0.5">
        {ATAJOS.map((a) => (
          <label
            key={a.id}
            className="relative flex h-8 cursor-pointer items-center rounded-md px-3 text-sm text-tenue hover:text-texto has-checked:bg-seleccion has-checked:font-semibold has-checked:text-acento has-focus-visible:outline-2 has-focus-visible:outline-acento"
          >
            <input
              type="radio"
              name="rango"
              className="sr-only"
              checked={rango.atajo === a.id}
              onChange={() => onCambiar(a.id === "personalizado" ? { ...rango, atajo: a.id } : rangoDe(a.id))}
            />
            {a.nombre}
          </label>
        ))}
      </div>
      {rango.atajo === "personalizado" && (
        <span className="flex items-center gap-2 text-sm">
          <label className="flex items-center gap-1 text-tenue">
            Desde
            <input
              type="date"
              value={rango.desde}
              max={rango.hasta}
              onChange={(e) => e.target.value && onCambiar({ ...rango, desde: e.target.value })}
              className="h-8 tarjeta px-2 text-texto"
            />
          </label>
          <label className="flex items-center gap-1 text-tenue">
            hasta
            <input
              type="date"
              value={rango.hasta}
              min={rango.desde}
              onChange={(e) => e.target.value && onCambiar({ ...rango, hasta: e.target.value })}
              className="h-8 tarjeta px-2 text-texto"
            />
          </label>
        </span>
      )}
    </div>
  );
}

type Resultado = { tipo: "ok"; ruta: string } | { tipo: "error"; texto: string } | { tipo: "trabajando" } | null;

/** Botones Excel y PDF del reporte. Ctrl+E exporta a Excel mientras la pestaña está activa. */
export function Exportar({
  activa,
  tipo,
  rango,
  categoria = null,
}: {
  activa: boolean;
  tipo: TipoReporte;
  rango: Rango;
  categoria?: number | null;
}) {
  const [resultado, setResultado] = useState<Resultado>(null);
  const exportar = useCallback(
    async (formato: FormatoExportacion) => {
      setResultado({ tipo: "trabajando" });
      try {
        const ruta = await api.exportarReporte(tipo, rango.desde, rango.hasta, categoria, formato);
        setResultado(ruta ? { tipo: "ok", ruta } : null);
      } catch (e) {
        setResultado({ tipo: "error", texto: textoError(e) });
      }
    },
    [tipo, rango, categoria],
  );
  useEffect(() => setResultado(null), [tipo, rango]);
  useAtajos("pantalla", { "Ctrl+E": () => void exportar("excel") }, activa);

  return (
    <div className="flex flex-wrap items-center gap-2">
      {resultado?.tipo === "ok" && (
        <span className="flex items-center gap-2 text-sm text-exito">
          <span className="max-w-72 truncate" title={resultado.ruta}>
            Guardado en {resultado.ruta}
          </span>
          <Boton variante="enlace" onClick={() => void api.abrirArchivo(resultado.ruta).catch(() => {})}>
            Abrir
          </Boton>
        </span>
      )}
      {resultado?.tipo === "error" && <span className="text-sm text-peligro">{resultado.texto}</span>}
      {resultado?.tipo === "trabajando" && <span className="text-sm text-tenue">Exportando…</span>}
      <Boton icono={FileSpreadsheet} atajo="Ctrl E" onClick={() => void exportar("excel")}>
        Excel
      </Boton>
      <Boton icono={FileDown} onClick={() => void exportar("pdf")}>
        PDF
      </Boton>
    </div>
  );
}

/** Carga un reporte cuando cambia el rango (descarta respuestas viejas si se cambia rápido). */
export function useReporte<T>(cargar: () => Promise<T>, deps: unknown[]): { datos: T | null; error: string | null } {
  const [datos, setDatos] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let vigente = true;
    setError(null);
    cargar()
      .then((d) => vigente && setDatos(d))
      .catch((e) => vigente && setError(textoError(e)));
    return () => {
      vigente = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
  return { datos, error };
}
