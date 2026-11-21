import { useVirtualizer } from "@tanstack/react-virtual";
import { X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  api,
  textoError,
  type CursorMovimientos,
  type MovimientoStock,
  type Producto,
  type TipoMovimientoStock,
} from "../../api";
import { cantidad, fechaHora, folio } from "../../formato";

const POR_PAGINA = 200;
const ALTO_FILA = 44;
const COLUMNAS =
  "grid-cols-[8.5rem_minmax(0,1fr)_7rem_6rem] @2xl:grid-cols-[9rem_minmax(8rem,1fr)_7.5rem_6.5rem_7rem_minmax(8rem,1fr)]";
const COL_ANCHA = "hidden @2xl:block";

export const TIPOS: { id: TipoMovimientoStock; nombre: string; clase: string }[] = [
  { id: "venta", nombre: "Venta", clase: "bg-acento/12 text-acento" },
  { id: "compra", nombre: "Compra", clase: "bg-exito-fondo text-exito" },
  { id: "ajuste", nombre: "Ajuste", clase: "bg-linea text-texto" },
  { id: "merma", nombre: "Merma", clase: "bg-peligro/10 text-peligro" },
  { id: "devolucion", nombre: "Devolución", clase: "bg-aviso-fondo text-aviso" },
];

interface Props {
  activa: boolean;
  /** Producto con que se abrió la pestaña (desde Stock o desde Productos). */
  producto: { id: number; nombre: string } | null;
  onProducto: (p: { id: number; nombre: string } | null) => void;
  onIrAVenta: (folio: number) => void;
  onIrACompra?: (compraId: number) => void;
}

/** Inventario > Movimientos: el historial de stock, del más nuevo al más antiguo. */
export default function Movimientos({ activa, producto, onProducto, onIrAVenta, onIrACompra }: Props) {
  const [tipo, setTipo] = useState<TipoMovimientoStock | "">("");
  const [desde, setDesde] = useState("");
  const [hasta, setHasta] = useState("");
  const [filas, setFilas] = useState<MovimientoStock[]>([]);
  const [siguiente, setSiguiente] = useState<CursorMovimientos | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [buscar, setBuscar] = useState("");
  const [sugeridos, setSugeridos] = useState<Producto[]>([]);
  const cargando = useRef(false);
  const consultaActual = useRef(0);
  const contenedor = useRef<HTMLDivElement>(null);

  const cargar = useCallback(
    async (despuesDe: CursorMovimientos | null) => {
      const consulta = despuesDe === null ? ++consultaActual.current : consultaActual.current;
      cargando.current = true;
      try {
        const p = await api.listarMovimientos(
          { productoId: producto?.id ?? null, tipo: tipo || null, desde: desde || null, hasta: hasta || null },
          despuesDe,
          POR_PAGINA,
        );
        if (consulta !== consultaActual.current) return;
        setFilas((f) => (despuesDe === null ? p.movimientos : [...f, ...p.movimientos]));
        setSiguiente(p.siguiente);
        setError(null);
      } catch (e) {
        setError(textoError(e));
      } finally {
        cargando.current = false;
      }
    },
    [producto, tipo, desde, hasta],
  );

  useEffect(() => {
    if (!activa) return;
    contenedor.current?.scrollTo({ top: 0 });
    void cargar(null);
  }, [activa, cargar]);

  const virtual = useVirtualizer({
    count: filas.length,
    getScrollElement: () => contenedor.current,
    estimateSize: () => ALTO_FILA,
    overscan: 10,
  });
  const visibles = virtual.getVirtualItems();
  const ultimaVisible = visibles.at(-1)?.index ?? 0;
  useEffect(() => {
    if (siguiente && !cargando.current && ultimaVisible >= filas.length - 30) void cargar(siguiente);
  }, [ultimaVisible, filas.length, siguiente, cargar]);

  const buscarProducto = async () => {
    const t = buscar.trim();
    if (!t) return;
    try {
      const porCodigo = await api.buscarProductoPorCodigo(t);
      if (porCodigo) {
        onProducto(porCodigo);
        setBuscar("");
        return;
      }
      const lista = await api.buscarProductos(t, true, 8, 0);
      if (lista.length === 1) {
        onProducto(lista[0]);
        setBuscar("");
      } else setSugeridos(lista);
    } catch (e) {
      setError(textoError(e));
    }
  };

  return (
    <section className="@container flex min-h-0 flex-1 flex-col">
      <div className="flex flex-wrap items-center gap-3">
        {producto ? (
          <span className="flex h-10 items-center gap-2 rounded-lg bg-acento/12 pr-1 pl-3 font-medium text-acento">
            {producto.nombre}
            <button
              type="button"
              onClick={() => onProducto(null)}
              aria-label="Ver todos los productos"
              className="flex size-8 items-center justify-center rounded-md hover:bg-acento/12"
            >
              <X size={16} aria-hidden />
            </button>
          </span>
        ) : (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void buscarProducto();
            }}
            className="relative min-w-56 flex-1"
          >
            <input
              value={buscar}
              onChange={(e) => {
                setBuscar(e.target.value);
                setSugeridos([]);
              }}
              placeholder="Filtrar por producto (nombre o código) y Enter"
              aria-label="Filtrar por producto"
              className="h-10 w-full rounded-lg border border-linea bg-superficie px-4 outline-none focus:border-texto"
            />
            {sugeridos.length > 0 && (
              <ul className="absolute z-10 mt-1 w-full overflow-hidden rounded-lg border border-linea bg-superficie">
                {sugeridos.map((p) => (
                  <li key={p.id}>
                    <button
                      type="button"
                      onClick={() => {
                        onProducto(p);
                        setSugeridos([]);
                        setBuscar("");
                      }}
                      className="h-10 w-full px-4 text-left hover:bg-fondo"
                    >
                      {p.nombre}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </form>
        )}
        <select
          value={tipo}
          onChange={(e) => setTipo(e.target.value as TipoMovimientoStock | "")}
          aria-label="Tipo de movimiento"
          className="h-10 rounded-lg border border-linea bg-superficie px-3 outline-none focus:border-texto"
        >
          <option value="">Todos los tipos</option>
          {TIPOS.map((t) => (
            <option key={t.id} value={t.id}>
              {t.nombre}
            </option>
          ))}
        </select>
        <label className="flex items-center gap-2 text-tenue">
          Desde
          <input
            type="date"
            value={desde}
            max={hasta || undefined}
            onChange={(e) => setDesde(e.target.value)}
            className="h-10 rounded-lg border border-linea bg-superficie px-3 text-texto outline-none focus:border-texto"
          />
        </label>
        <label className="flex items-center gap-2 text-tenue">
          Hasta
          <input
            type="date"
            value={hasta}
            min={desde || undefined}
            onChange={(e) => setHasta(e.target.value)}
            className="h-10 rounded-lg border border-linea bg-superficie px-3 text-texto outline-none focus:border-texto"
          />
        </label>
      </div>
      {error && <p className="mt-3 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}

      <div className={`mt-4 grid ${COLUMNAS} gap-2 px-4 text-xs font-medium uppercase text-tenue`}>
        <span>Fecha</span>
        <span>Producto</span>
        <span className="text-right">Cantidad</span>
        <span className="text-right">Stock</span>
        <span className={COL_ANCHA}>Tipo</span>
        <span className={COL_ANCHA}>Motivo u origen</span>
      </div>
      <div ref={contenedor} className="mt-1 min-h-0 flex-1 overflow-auto rounded-lg border border-linea bg-superficie tabular-nums">
        {filas.length === 0 ? (
          <p className="p-8 text-center text-tenue">No hay movimientos con estos filtros.</p>
        ) : (
          <div style={{ height: virtual.getTotalSize(), position: "relative" }}>
            {visibles.map((v) => {
              const m = filas[v.index];
              const t = TIPOS.find((x) => x.id === m.tipo) ?? TIPOS[2];
              return (
                <div
                  key={m.id}
                  style={{ position: "absolute", top: 0, left: 0, width: "100%", height: ALTO_FILA, transform: `translateY(${v.start}px)` }}
                  className={`grid ${COLUMNAS} items-center gap-2 border-b border-linea px-4`}
                >
                  <span className="text-tenue">{fechaHora(m.fecha)}</span>
                  <span className="truncate" title={m.producto}>
                    {m.producto}
                  </span>
                  <span className={`text-right font-medium ${m.cantidadMil > 0 ? "text-exito" : m.cantidadMil < 0 ? "text-peligro" : "text-tenue"}`}>
                    {m.cantidadMil > 0 ? "+" : m.cantidadMil < 0 ? "−" : ""}
                    {cantidad(Math.abs(m.cantidadMil), m.unidad)}
                  </span>
                  <span className="text-right">{cantidad(m.stockResultanteMil, m.unidad)}</span>
                  <span className={COL_ANCHA}>
                    <span className={`inline-flex h-6 items-center rounded-full px-2.5 text-xs font-medium ${t.clase}`}>{t.nombre}</span>
                  </span>
                  <span className={`${COL_ANCHA} min-w-0 truncate text-tenue`} title={[m.motivo, m.nota].filter(Boolean).join(" · ")}>
                    {m.folio !== null ? (
                      <button type="button" onClick={() => onIrAVenta(m.folio!)} className="text-acento hover:underline">
                        Venta {folio(m.folio)}
                      </button>
                    ) : m.compraId !== null && onIrACompra ? (
                      <button type="button" onClick={() => onIrACompra(m.compraId!)} className="text-acento hover:underline">
                        Compra N° {m.compraId}
                      </button>
                    ) : null}
                    {(m.folio !== null || m.compraId !== null) && (m.motivo || m.nota) ? " · " : ""}
                    {m.motivo ?? m.nota}
                  </span>
                </div>
              );
            })}
          </div>
        )}
      </div>
      <p className="mt-2 px-1 text-xs text-tenue">
        {filas.length} {filas.length === 1 ? "movimiento" : "movimientos"}
        {siguiente ? " cargados (hay más al bajar)" : ""}
      </p>
    </section>
  );
}
