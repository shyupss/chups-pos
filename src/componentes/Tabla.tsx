import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowDown, ArrowUp, ChevronRight } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import Boton from "./Boton";
import { columnasVisibles, ordenarFilas, plantillaGrid, siguienteOrden, type Orden } from "./tabla-nucleo";

export interface ColumnaTabla<T> {
  /** Identifica la columna para ordenar. */
  id: string;
  titulo: string;
  /** Pista del grid: "7rem", "minmax(10rem,28rem)"… El nombre no se estira sin límite (máximo ~28rem). */
  ancho: string;
  /** Los números (precio, stock, total, cantidad) van a la derecha y en cifras tabulares. */
  alinear?: "izquierda" | "derecha" | "centro";
  /** Se oculta cuando la tabla mide menos que esto (px CSS): primero lo secundario (código, categoría). */
  ocultarBajo?: number;
  celda: (fila: T) => ReactNode;
  /** Valor para ordenar al hacer clic en el encabezado. Sin esto la columna no se ordena. */
  orden?: (fila: T) => string | number | null | undefined;
  /** Clases de la celda que dependen de la fila (color, peso). */
  clase?: (fila: T) => string;
  /** Deja que el contenido se corte solo si hace falta, sin "…" (por ejemplo, un nombre con una insignia al lado). */
  sinPuntos?: boolean;
}

interface Props<T> {
  /** Para lectores de pantalla: "Productos", "Ventas"… */
  etiqueta: string;
  columnas: ColumnaTabla<T>[];
  filas: T[];
  clave: (fila: T) => string | number;
  /** Con esto la fila se abre al hacer clic (o Enter) y muestra una flecha "›" al final. */
  onFila?: (fila: T) => void;
  seleccionada?: (fila: T) => boolean;
  /** Clases extra de la fila (por ejemplo, tachada si está desactivada). */
  claseFila?: (fila: T) => string;
  cargando?: boolean;
  /** Estado vacío: una frase útil y, si corresponde, el botón de la acción. */
  vacio?: { texto: string; accion?: { etiqueta: string; onClick: () => void } };
  /** El conteo de abajo: textoConteo(...) de tabla-nucleo. */
  pie?: ReactNode;
  /** Se llama al acercarse al final de lo cargado: pide la página siguiente. */
  onCercaDelFinal?: () => void;
  /**
   * Agrupa las filas con un encabezado (por ejemplo, cada día en Ventas). Las filas deben venir ya en el
   * orden del grupo; al ordenar por una columna los encabezados se quitan.
   */
  grupo?: (fila: T) => { clave: string; texto: string };
  /** Cuando cambia, la tabla vuelve arriba (una búsqueda o un filtro nuevos). */
  reiniciar?: unknown;
  /** Cuando cambia, la tabla baja hasta la última fila (se agregó una línea al final de una lista que se edita). */
  irAlFinal?: unknown;
  altoFila?: number;
  /** Una tabla corta que crece hasta este alto (px) y luego se desplaza, en vez de llenar el espacio que le dan. */
  alto?: number;
  /** Clases de afuera (flex-1, márgenes). */
  className?: string;
}

const ALTO_FILA = 48;
const ALTO_GRUPO = 36;

type Item<T> = { tipo: "grupo"; clave: string; texto: string } | { tipo: "fila"; fila: T };

function useAncho(ref: React.RefObject<HTMLElement | null>) {
  const [ancho, setAncho] = useState(0);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const observador = new ResizeObserver(([e]) => setAncho(Math.round(e.contentRect.width)));
    observador.observe(el);
    return () => observador.disconnect();
  }, [ref]);
  return ancho;
}

/**
 * La tabla de toda la app, sobre @tanstack/react-virtual: solo se dibujan las filas que se ven.
 * Encabezado fijo dentro de la tarjeta, filas de 48 px, columnas declaradas (título, ancho, alineación,
 * cuándo se oculta), orden al hacer clic en el encabezado (de lo ya cargado: el backend no ordena),
 * conteo al pie y estado vacío con su acción.
 */
export default function Tabla<T>({
  etiqueta,
  columnas,
  filas,
  clave,
  onFila,
  seleccionada,
  claseFila,
  cargando,
  vacio,
  pie,
  onCercaDelFinal,
  grupo,
  reiniciar,
  irAlFinal,
  altoFila = ALTO_FILA,
  alto,
  className = "",
}: Props<T>) {
  const contenedor = useRef<HTMLDivElement>(null);
  const ancho = useAncho(contenedor);
  const [orden, setOrden] = useState<Orden | null>(null);

  const visibles = useMemo(() => columnasVisibles(columnas, ancho), [columnas, ancho]);
  const grid = plantillaGrid(
    visibles.map((c) => c.ancho),
    Boolean(onFila),
  );

  const ordenadas = useMemo(() => {
    const col = orden && columnas.find((c) => c.id === orden.columna);
    return col?.orden && orden ? ordenarFilas(filas, col.orden, orden.direccion) : filas;
  }, [filas, columnas, orden]);

  useEffect(() => {
    contenedor.current?.scrollTo({ top: 0 });
  }, [reiniciar]);

  // Los encabezados de grupo son ítems de la misma lista virtual: no se carga nada de más.
  const items = useMemo<Item<T>[]>(() => {
    if (!grupo || orden) return ordenadas.map((fila) => ({ tipo: "fila", fila }));
    const lista: Item<T>[] = [];
    let anterior = "";
    for (const fila of ordenadas) {
      const g = grupo(fila);
      if (g.clave !== anterior) {
        lista.push({ tipo: "grupo", clave: g.clave, texto: g.texto });
        anterior = g.clave;
      }
      lista.push({ tipo: "fila", fila });
    }
    return lista;
  }, [ordenadas, grupo, orden]);

  const virtual = useVirtualizer({
    count: items.length,
    getScrollElement: () => contenedor.current,
    estimateSize: (i) => (items[i]?.tipo === "grupo" ? ALTO_GRUPO : altoFila),
    overscan: 10,
  });
  // Las alturas dependen del tipo de ítem: si la lista cambia, se vuelven a calcular.
  useEffect(() => virtual.measure(), [items, virtual]);
  useEffect(() => {
    if (irAlFinal !== undefined && items.length > 0) virtual.scrollToIndex(items.length - 1);
    // Solo cuando `irAlFinal` cambia: no cada vez que cambian las filas.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [irAlFinal]);
  const dibujadas = virtual.getVirtualItems();
  const ultima = dibujadas.at(-1)?.index ?? 0;
  useEffect(() => {
    if (onCercaDelFinal && items.length > 0 && ultima >= items.length - 20) onCercaDelFinal();
  }, [ultima, items.length, onCercaDelFinal]);

  const alinear = (c: ColumnaTabla<T>) =>
    c.alinear === "derecha" ? "text-right tabular-nums" : c.alinear === "centro" ? "text-center" : "text-left";

  return (
    <div className={`tarjeta flex min-h-0 flex-col overflow-hidden ${className}`} style={alto ? { maxHeight: alto } : undefined}>
      <div
        ref={contenedor}
        role="table"
        aria-label={etiqueta}
        aria-rowcount={ordenadas.length}
        className={`min-h-0 overflow-auto ${alto ? "flex-auto" : "flex-1"}`}
      >
        <div
          role="row"
          style={{ gridTemplateColumns: grid }}
          className="sticky top-0 z-10 grid h-11 items-center gap-4 border-b border-linea bg-suave px-4 text-[13px] font-semibold text-tenue"
        >
          {visibles.map((c) => {
            const activa = orden?.columna === c.id;
            const flecha = activa && orden ? orden.direccion === "asc" ? <ArrowUp size={14} aria-hidden /> : <ArrowDown size={14} aria-hidden /> : null;
            const posicion = c.alinear === "derecha" ? "justify-end" : c.alinear === "centro" ? "justify-center" : "justify-start";
            return c.orden ? (
              <button
                key={c.id}
                type="button"
                role="columnheader"
                aria-sort={activa && orden ? (orden.direccion === "asc" ? "ascending" : "descending") : "none"}
                title={`Ordenar por ${c.titulo.toLowerCase()}`}
                onClick={() => setOrden((o) => siguienteOrden(o, c.id))}
                className={`-mx-2 flex h-8 min-w-0 cursor-pointer items-center gap-1 rounded-md px-2 hover:bg-linea hover:text-texto ${posicion} ${
                  activa ? "text-texto" : ""
                }`}
              >
                <span className="truncate">{c.titulo}</span>
                {flecha}
              </button>
            ) : (
              <span key={c.id} role="columnheader" className={`flex min-w-0 items-center ${posicion}`}>
                <span className="truncate">{c.titulo}</span>
              </span>
            );
          })}
          {onFila && <span role="presentation" />}
        </div>

        {ordenadas.length === 0 ? (
          <div className="flex flex-col items-center gap-4 px-6 py-12 text-center text-tenue">
            <p>{cargando ? "Cargando…" : (vacio?.texto ?? "No hay nada que mostrar.")}</p>
            {!cargando && vacio?.accion && (
              <Boton variante="primario" onClick={vacio.accion.onClick}>
                {vacio.accion.etiqueta}
              </Boton>
            )}
          </div>
        ) : (
          <div style={{ height: virtual.getTotalSize(), position: "relative" }}>
            {dibujadas.map((v) => {
              const item = items[v.index];
              if (!item) return null;
              if (item.tipo === "grupo") {
                return (
                  <div
                    key={`grupo-${item.clave}`}
                    role="row"
                    style={{ position: "absolute", top: 0, left: 0, width: "100%", height: ALTO_GRUPO, transform: `translateY(${v.start}px)` }}
                    className="flex items-center border-b border-linea bg-suave px-4 text-[13px] font-semibold text-tenue"
                  >
                    <span role="columnheader">{item.texto}</span>
                  </div>
                );
              }
              const fila = item.fila;
              const elegida = seleccionada?.(fila) ?? false;
              return (
                <div
                  key={clave(fila)}
                  role="row"
                  aria-rowindex={v.index + 2}
                  aria-selected={onFila ? elegida : undefined}
                  tabIndex={onFila ? 0 : undefined}
                  onClick={onFila ? () => onFila(fila) : undefined}
                  onKeyDown={
                    onFila
                      ? (e) => {
                          if (e.target === e.currentTarget && (e.key === "Enter" || e.key === " ")) {
                            e.preventDefault();
                            onFila(fila);
                          }
                        }
                      : undefined
                  }
                  style={{ gridTemplateColumns: grid, position: "absolute", top: 0, left: 0, width: "100%", height: altoFila, transform: `translateY(${v.start}px)` }}
                  className={`grid items-center gap-4 border-b border-linea px-4 ${
                    elegida ? "bg-seleccion" : onFila ? "hover:bg-suave" : ""
                  } ${onFila ? "cursor-pointer" : ""} ${claseFila?.(fila) ?? ""}`}
                >
                  {visibles.map((c) => (
                    <div
                      key={c.id}
                      role="cell"
                      className={`min-w-0 ${c.sinPuntos ? "" : "truncate"} ${alinear(c)} ${c.clase?.(fila) ?? ""}`}
                    >
                      {c.celda(fila)}
                    </div>
                  ))}
                  {onFila && <ChevronRight size={20} aria-hidden className="justify-self-end text-tenue" />}
                </div>
              );
            })}
          </div>
        )}
      </div>
      {pie && <div className="flex h-10 shrink-0 items-center border-t border-linea px-4 text-[13px] text-tenue tabular-nums">{pie}</div>}
    </div>
  );
}
