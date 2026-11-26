import Boton from "../../componentes/Boton";
import { X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  api,
  textoError,
  type CursorMovimientos,
  type MovimientoStock,
  type Producto,
  type TipoMovimientoStock,
} from "../../api";
import Insignia, { type TonoInsignia } from "../../componentes/Insignia";
import Tabla, { type ColumnaTabla } from "../../componentes/Tabla";
import { textoConteo } from "../../componentes/tabla-nucleo";
import { cantidad, fechaHora, folio } from "../../formato";

const POR_PAGINA = 200;

export const TIPOS: { id: TipoMovimientoStock; nombre: string; tono: TonoInsignia }[] = [
  { id: "venta", nombre: "Venta", tono: "info" },
  { id: "compra", nombre: "Compra", tono: "exito" },
  { id: "ajuste", nombre: "Ajuste", tono: "neutro" },
  { id: "merma", nombre: "Merma", tono: "peligro" },
  { id: "devolucion", nombre: "Devolución", tono: "aviso" },
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
    void cargar(null);
  }, [activa, cargar]);

  const cargarMas = useCallback(() => {
    if (siguiente && !cargando.current) void cargar(siguiente);
  }, [siguiente, cargar]);

  // Fecha · Producto · Cantidad · Stock · Tipo · Origen: qué pasó y cuándo, cuánto cambió, y de dónde viene.
  const columnas = useMemo<ColumnaTabla<MovimientoStock>[]>(
    () => [
      { id: "fecha", titulo: "Fecha", ancho: "9.5rem", orden: (m) => m.fecha, clase: () => "text-tenue", celda: (m) => fechaHora(m.fecha) },
      { id: "producto", titulo: "Producto", ancho: "minmax(10rem,24rem)", orden: (m) => m.producto, celda: (m) => <span title={m.producto}>{m.producto}</span> },
      {
        id: "cantidad",
        titulo: "Cantidad",
        ancho: "7.5rem",
        alinear: "derecha",
        orden: (m) => m.cantidadMil,
        clase: (m) => `font-medium ${m.cantidadMil > 0 ? "text-exito" : m.cantidadMil < 0 ? "text-peligro" : "text-tenue"}`,
        celda: (m) => (
          <>
            {m.cantidadMil > 0 ? "+" : m.cantidadMil < 0 ? "−" : ""}
            {cantidad(Math.abs(m.cantidadMil), m.unidad)}
          </>
        ),
      },
      { id: "stock", titulo: "Stock", ancho: "6.5rem", alinear: "derecha", orden: (m) => m.stockResultanteMil, celda: (m) => cantidad(m.stockResultanteMil, m.unidad) },
      {
        id: "tipo",
        titulo: "Tipo",
        ancho: "8.5rem",
        ocultarBajo: 760,
        sinPuntos: true,
        orden: (m) => m.tipo,
        celda: (m) => {
          const t = TIPOS.find((x) => x.id === m.tipo) ?? TIPOS[2];
          return <Insignia tono={t.tono}>{t.nombre}</Insignia>;
        },
      },
      {
        id: "origen",
        titulo: "Motivo u origen",
        ancho: "minmax(10rem,18rem)",
        ocultarBajo: 960,
        clase: () => "text-tenue",
        celda: (m) => (
          <span title={[m.motivo, m.nota].filter(Boolean).join(" · ")}>
            {m.folio !== null ? (
              <Boton variante="enlace" onClick={() => onIrAVenta(m.folio!)}>
                Venta {folio(m.folio)}
              </Boton>
            ) : m.compraId !== null && onIrACompra ? (
              <Boton variante="enlace" onClick={() => onIrACompra(m.compraId!)}>
                Compra N° {m.compraId}
              </Boton>
            ) : null}
            {/* En una compra la nota es "Compra N° x", que el enlace ya dice. */}
            {(m.folio !== null || m.compraId !== null) && (m.compraId !== null ? m.motivo : m.motivo || m.nota) ? " · " : ""}
            {m.compraId !== null ? m.motivo : (m.motivo ?? m.nota)}
          </span>
        ),
      },
    ],
    [onIrAVenta, onIrACompra],
  );

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
          <span className="flex h-11 items-center gap-2 rounded-lg bg-seleccion pr-1 pl-3 font-medium text-acento">
            {producto.nombre}
            <Boton variante="terciario" tamano="icono" onClick={() => onProducto(null)} aria-label="Ver todos los productos" className="size-7">
              <X size={16} aria-hidden />
            </Boton>
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
              className="h-11 w-full rounded-lg border border-borde-control bg-superficie px-4 focus:border-acento"
            />
            {sugeridos.length > 0 && (
              <ul className="absolute z-10 mt-1 w-full overflow-hidden tarjeta shadow-s2">
                {sugeridos.map((p) => (
                  <li key={p.id}>
                    <button
                      type="button"
                      onClick={() => {
                        onProducto(p);
                        setSugeridos([]);
                        setBuscar("");
                      }}
                      className="h-11 w-full px-4 text-left hover:bg-suave"
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
          className="h-11 rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento"
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
            className="h-11 rounded-lg border border-borde-control bg-superficie px-3 text-texto focus:border-acento"
          />
        </label>
        <label className="flex items-center gap-2 text-tenue">
          Hasta
          <input
            type="date"
            value={hasta}
            min={desde || undefined}
            onChange={(e) => setHasta(e.target.value)}
            className="h-11 rounded-lg border border-borde-control bg-superficie px-3 text-texto focus:border-acento"
          />
        </label>
      </div>
      {error && <p className="mt-3 rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}

      <Tabla
        etiqueta="Movimientos de stock"
        className="mt-4 flex-1"
        columnas={columnas}
        filas={filas}
        clave={(m) => m.id}
        reiniciar={`${producto?.id ?? ""}|${tipo}|${desde}|${hasta}`}
        onCercaDelFinal={cargarMas}
        pie={textoConteo(filas.length, "movimiento", "movimientos", siguiente !== null)}
        vacio={{ texto: "No hay movimientos con estos filtros." }}
      />
    </section>
  );
}
