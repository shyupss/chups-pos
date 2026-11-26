import Boton from "../../componentes/Boton";
import { ChevronDown, Download, FileSpreadsheet, FileText } from "lucide-react";
import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useAtajos } from "../../atajos";
import { api, textoError, type FormatoExportacion, type TipoReporte } from "../../api";
import Tecla from "../../componentes/Tecla";
import { carpetaDe } from "../../formato";
import { periodoEnPalabras, rangoDe, type Atajo, type Rango } from "./reportes-nucleo";

export { rangoDe };
export type { Atajo, Rango };

// Lo habitual a la vista; lo demás (ayer, últimos 30 días, este año) dentro de "Más".
const PRINCIPALES: { id: Atajo; nombre: string }[] = [
  { id: "mes", nombre: "Este mes" },
  { id: "mesPasado", nombre: "Mes pasado" },
  { id: "semana", nombre: "Esta semana" },
  { id: "hoy", nombre: "Hoy" },
  { id: "personalizado", nombre: "Elegir fechas…" },
];
const MAS: { id: Atajo; nombre: string }[] = [
  { id: "ayer", nombre: "Ayer" },
  { id: "ultimos30", nombre: "Últimos 30 días" },
  { id: "anio", nombre: "Este año" },
];

const claseOpcion =
  "relative flex h-9 cursor-pointer items-center gap-1 rounded-md px-3 text-tenue hover:text-texto has-checked:bg-seleccion has-checked:font-semibold has-checked:text-acento has-focus-visible:outline-2 has-focus-visible:outline-acento";

/** Cierra algo flotante al hacer clic fuera o apretar Esc. */
function useCerrarAfuera(abierto: boolean, cerrar: () => void) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!abierto) return;
    const fuera = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) cerrar();
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && cerrar();
    document.addEventListener("mousedown", fuera);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", fuera);
      document.removeEventListener("keydown", esc);
    };
  }, [abierto, cerrar]);
  return ref;
}

/** El rango de fechas: lo de siempre a la vista ("Este mes" por defecto), el resto en "Más" y "Elegir fechas…". */
export function SelectorRango({ rango, onCambiar }: { rango: Rango; onCambiar: (r: Rango) => void }) {
  const [masAbierto, setMasAbierto] = useState(false);
  const cerrarMas = useCallback(() => setMasAbierto(false), []);
  const refMas = useCerrarAfuera(masAbierto, cerrarMas);
  const enMas = MAS.find((a) => a.id === rango.atajo);
  const elegir = (id: Atajo) => onCambiar(id === "personalizado" ? { ...rango, atajo: id } : rangoDe(id));

  return (
    <div className="flex flex-wrap items-center gap-2">
      <div role="radiogroup" aria-label="Período" className="flex flex-wrap gap-1 rounded-lg border border-borde-control bg-suave p-0.5">
        {PRINCIPALES.map((a) => (
          <label key={a.id} className={claseOpcion}>
            <input type="radio" name="rango" className="sr-only" checked={rango.atajo === a.id} onChange={() => elegir(a.id)} />
            {a.nombre}
          </label>
        ))}
      </div>
      <div ref={refMas} className="relative">
        <Boton
          variante={enMas ? "secundario" : "terciario"}
          icono={ChevronDown}
          aria-haspopup="menu"
          aria-expanded={masAbierto}
          onClick={() => setMasAbierto((v) => !v)}
          className={enMas ? "border-acento bg-seleccion font-semibold text-acento" : ""}
        >
          {enMas ? enMas.nombre : "Más"}
        </Boton>
        {masAbierto && (
          <ul role="menu" aria-label="Más períodos" className="absolute left-0 z-20 mt-1 w-48 overflow-hidden tarjeta shadow-s2">
            {MAS.map((a) => (
              <li key={a.id} role="none">
                <button
                  type="button"
                  role="menuitemradio"
                  aria-checked={rango.atajo === a.id}
                  onClick={() => {
                    elegir(a.id);
                    setMasAbierto(false);
                  }}
                  className={`flex h-11 w-full items-center px-4 text-left hover:bg-suave ${rango.atajo === a.id ? "bg-seleccion font-semibold text-acento" : ""}`}
                >
                  {a.nombre}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
      {rango.atajo === "personalizado" && (
        <span className="flex flex-wrap items-center gap-2">
          <label className="flex items-center gap-2 text-tenue">
            Desde
            <input
              type="date"
              value={rango.desde}
              max={rango.hasta}
              onChange={(e) => e.target.value && onCambiar({ ...rango, desde: e.target.value })}
              className="h-11 tarjeta px-2 text-texto"
            />
          </label>
          <label className="flex items-center gap-2 text-tenue">
            hasta
            <input
              type="date"
              value={rango.hasta}
              min={rango.desde}
              onChange={(e) => e.target.value && onCambiar({ ...rango, hasta: e.target.value })}
              className="h-11 tarjeta px-2 text-texto"
            />
          </label>
        </span>
      )}
    </div>
  );
}

/** El período del reporte dicho en palabras ("Noviembre 2026", "Del 3 al 17 de noviembre"), como título. */
export function TituloPeriodo({ rango }: { rango: Rango }) {
  return <h2 className="text-xl font-semibold">{periodoEnPalabras(rango.desde, rango.hasta)}</h2>;
}

/**
 * Dónde se dibuja "Descargar este reporte": la fila del título, a la derecha, para que las tarjetas tengan todo el
 * ancho. Reportes lo fija; sin él, el botón queda donde se declara.
 */
export const SlotExportar = createContext<HTMLElement | null>(null);

type Resultado = { tipo: "ok"; ruta: string } | { tipo: "error"; texto: string } | { tipo: "trabajando" } | null;

/**
 * "Descargar este reporte": un solo botón con un menú de dos opciones explicadas (Excel y PDF), lo que incluye
 * el archivo y, al terminar, dónde quedó con "Abrir" y "Mostrar en la carpeta". Ctrl+E descarga el Excel.
 */
export function Exportar({
  activa,
  tipo,
  rango,
  nombre,
  categoria = null,
}: {
  activa: boolean;
  tipo: TipoReporte;
  rango: Rango;
  /** El nombre del reporte, tal como se ve en la pestaña: "Ventas", "Productos"… */
  nombre: string;
  categoria?: number | null;
}) {
  const [resultado, setResultado] = useState<Resultado>(null);
  const [abierto, setAbierto] = useState(false);
  const cerrar = useCallback(() => setAbierto(false), []);
  const ref = useCerrarAfuera(abierto, cerrar);
  const exportar = useCallback(
    async (formato: FormatoExportacion) => {
      setAbierto(false);
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

  const periodo = periodoEnPalabras(rango.desde, rango.hasta);
  const enMinuscula = periodo.charAt(0).toLowerCase() + periodo.slice(1);
  const slot = useContext(SlotExportar);

  const bloque = (
    <div className="flex max-w-md flex-col items-end gap-1 text-right">
      <div ref={ref} className="relative">
        <Boton icono={Download} aria-haspopup="menu" aria-expanded={abierto} disabled={resultado?.tipo === "trabajando"} onClick={() => setAbierto((v) => !v)}>
          {resultado?.tipo === "trabajando" ? "Descargando…" : "Descargar este reporte"}
          <ChevronDown size={16} aria-hidden />
        </Boton>
        {abierto && (
          <div role="menu" aria-label="Descargar este reporte" className="absolute right-0 z-20 mt-1 w-96 max-w-[80vw] overflow-hidden tarjeta text-left shadow-s2">
            <button type="button" role="menuitem" onClick={() => void exportar("excel")} className="flex w-full items-start gap-3 px-4 py-3 text-left hover:bg-suave">
              <FileSpreadsheet size={22} aria-hidden className="mt-0.5 shrink-0 text-acento" />
              <span className="min-w-0 flex-1">
                <span className="flex items-center gap-2 font-semibold">
                  Planilla Excel <Tecla>Ctrl E</Tecla>
                </span>
                <span className="block text-sm text-tenue">Para revisar los números o mandarlos al contador.</span>
              </span>
            </button>
            <button type="button" role="menuitem" onClick={() => void exportar("pdf")} className="flex w-full items-start gap-3 border-t border-linea px-4 py-3 text-left hover:bg-suave">
              <FileText size={22} aria-hidden className="mt-0.5 shrink-0 text-acento" />
              <span className="min-w-0 flex-1">
                <span className="font-semibold">PDF</span>
                <span className="block text-sm text-tenue">Para imprimir o enviar por WhatsApp o correo.</span>
              </span>
            </button>
          </div>
        )}
      </div>
      <p className="text-[13px] text-tenue">
        Incluye lo que ves en pantalla: {nombre}, {enMinuscula}.
      </p>
      {resultado?.tipo === "ok" && (
        <div role="status" className="flex flex-col items-end gap-2">
          <span className="max-w-full truncate text-sm text-exito" title={resultado.ruta}>
            Guardado en {carpetaDe(resultado.ruta)}
          </span>
          <span className="flex gap-2">
            <Boton variante="terciario" tamano="chico" onClick={() => void api.abrirArchivo(resultado.ruta).catch(() => {})}>
              Abrir
            </Boton>
            <Boton variante="terciario" tamano="chico" onClick={() => void api.mostrarEnCarpeta(resultado.ruta).catch(() => {})}>
              Mostrar en la carpeta
            </Boton>
          </span>
        </div>
      )}
      {resultado?.tipo === "error" && (
        <span role="alert" className="text-sm text-peligro">
          {resultado.texto}
        </span>
      )}
    </div>
  );
  return slot ? createPortal(bloque, slot) : bloque;
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
