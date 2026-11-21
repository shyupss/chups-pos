import { useVirtualizer } from "@tanstack/react-virtual";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, textoError, type Categoria, type FiltroCategoria, type Producto } from "../api";
import FormularioProducto from "../componentes/FormularioProducto";
import PanelCategorias from "../componentes/PanelCategorias";
import { cantidad, pesos } from "../formato";

interface Props {
  codigoNuevo: string | null;
  onCodigoUsado: () => void;
}

const POR_PAGINA = 100;
const ALTO_FILA = 40;
const COLUMNAS = "grid-cols-[minmax(8rem,1fr)_8rem_9rem_7rem_6rem]";

type Panel =
  | { tipo: "nuevo"; codigo?: string }
  | { tipo: "editar"; producto: Producto }
  | { tipo: "categorias" }
  | null;

/** Valor del <select> de categoría: "todas", "sin" o el id como texto. */
const aFiltro = (valor: string): FiltroCategoria => (valor === "todas" || valor === "sin" ? valor : Number(valor));

export default function Productos({ codigoNuevo, onCodigoUsado }: Props) {
  const [texto, setTexto] = useState("");
  const [busqueda, setBusqueda] = useState("");
  const [inactivos, setInactivos] = useState(false);
  const [filtro, setFiltro] = useState<FiltroCategoria>("todas");
  const [categorias, setCategorias] = useState<Categoria[]>([]);
  const [filas, setFilas] = useState<Producto[]>([]);
  const [hayMas, setHayMas] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [panel, setPanel] = useState<Panel>(null);
  const cargando = useRef(false);
  const consultaActual = useRef(0);
  const contenedor = useRef<HTMLDivElement>(null);

  const nombreCategoria = useMemo(() => new Map(categorias.map((c) => [c.id, c.nombre])), [categorias]);

  const cargarCategorias = useCallback(async () => {
    try {
      const lista = await api.listarCategorias();
      setCategorias(lista);
      // Si se eliminó la categoría del filtro, vuelve a "Todas".
      setFiltro((f) => (typeof f === "number" && !lista.some((c) => c.id === f) ? "todas" : f));
    } catch (e) {
      setError(textoError(e));
    }
  }, []);

  useEffect(() => {
    void cargarCategorias();
  }, [cargarCategorias]);

  useEffect(() => {
    if (codigoNuevo) {
      setPanel({ tipo: "nuevo", codigo: codigoNuevo });
      onCodigoUsado();
    }
  }, [codigoNuevo, onCodigoUsado]);

  // Espera a que el usuario deje de escribir antes de consultar.
  useEffect(() => {
    const t = setTimeout(() => setBusqueda(texto), 150);
    return () => clearTimeout(t);
  }, [texto]);

  const cargar = useCallback(
    async (desde: number) => {
      const consulta = desde === 0 ? ++consultaActual.current : consultaActual.current;
      cargando.current = true;
      try {
        const pagina = await api.buscarProductos(busqueda, inactivos, POR_PAGINA, desde, filtro);
        if (consulta !== consultaActual.current) return; // llegó tarde, ya se buscó otra cosa
        setFilas((f) => (desde === 0 ? pagina : [...f, ...pagina]));
        setHayMas(pagina.length === POR_PAGINA);
        setError(null);
      } catch (e) {
        setError(textoError(e));
      } finally {
        cargando.current = false;
      }
    },
    [busqueda, inactivos, filtro],
  );

  useEffect(() => {
    contenedor.current?.scrollTo({ top: 0 });
    void cargar(0);
  }, [cargar]);

  const virtual = useVirtualizer({
    count: filas.length,
    getScrollElement: () => contenedor.current,
    estimateSize: () => ALTO_FILA,
    overscan: 10,
  });
  const visibles = virtual.getVirtualItems();
  const ultimaVisible = visibles.at(-1)?.index ?? 0;

  // Carga la siguiente página al acercarse al final.
  useEffect(() => {
    if (hayMas && !cargando.current && ultimaVisible >= filas.length - 20 && filas.length > 0) {
      void cargar(filas.length);
    }
  }, [ultimaVisible, filas.length, hayMas, cargar]);

  const alGuardar = (p: Producto) => {
    setFilas((f) => {
      const i = f.findIndex((x) => x.id === p.id);
      if (i === -1) return [p, ...f];
      const copia = f.slice();
      copia[i] = p;
      return copia;
    });
    setPanel({ tipo: "editar", producto: p });
    // Los contadores de productos por categoría pueden haber cambiado.
    void cargarCategorias();
  };

  return (
    <div className="flex h-full">
      <section className="flex min-w-0 flex-1 flex-col p-6">
        <div className="flex flex-wrap items-center gap-3">
          <input
            value={texto}
            onChange={(e) => setTexto(e.target.value)}
            placeholder="Buscar por nombre o código"
            className="h-10 min-w-48 flex-1 rounded-lg border border-linea bg-superficie px-4 outline-none focus:border-texto"
            autoFocus
          />
          <select
            value={String(filtro)}
            onChange={(e) => setFiltro(aFiltro(e.target.value))}
            aria-label="Filtrar por categoría"
            className="h-10 max-w-48 rounded-lg border border-linea bg-superficie px-3 outline-none focus:border-texto"
          >
            <option value="todas">Todas las categorías</option>
            {categorias.map((c) => (
              <option key={c.id} value={c.id}>
                {c.nombre}
              </option>
            ))}
            <option value="sin">Sin categoría</option>
          </select>
          <button
            onClick={() => setPanel({ tipo: "categorias" })}
            className="h-10 rounded-lg border border-linea px-4 hover:bg-superficie"
          >
            Categorías
          </button>
          <label className="flex items-center gap-2 text-tenue">
            <input type="checkbox" checked={inactivos} onChange={(e) => setInactivos(e.target.checked)} />
            Ver desactivados
          </label>
          <button
            onClick={() => setPanel({ tipo: "nuevo" })}
            className="h-10 rounded-lg bg-acento px-4 font-semibold text-sobre-acento"
          >
            Nuevo producto
          </button>
        </div>

        {error && <p className="mt-3 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}

        <div className={`mt-4 grid ${COLUMNAS} gap-2 px-4 text-xs font-medium uppercase text-tenue`}>
          <span>Nombre</span>
          <span>Categoría</span>
          <span>Código</span>
          <span className="text-right">Precio</span>
          <span className="text-right">Stock</span>
        </div>
        <div ref={contenedor} className="mt-1 min-h-0 flex-1 overflow-auto rounded-lg border border-linea bg-superficie tabular-nums">
          {filas.length === 0 ? (
            <p className="p-8 text-center text-tenue">
              {busqueda || filtro !== "todas"
                ? "Sin resultados"
                : "Todavía no hay productos. Crea el primero con \"Nuevo producto\"."}
            </p>
          ) : (
            <div style={{ height: virtual.getTotalSize(), position: "relative" }}>
              {visibles.map((v) => {
                const p = filas[v.index];
                const bajo = p.stockMinimoMil > 0 && p.stockMil <= p.stockMinimoMil;
                const seleccionado = panel?.tipo === "editar" && panel.producto.id === p.id;
                return (
                  <button
                    key={p.id}
                    onClick={() => setPanel({ tipo: "editar", producto: p })}
                    style={{ position: "absolute", top: 0, left: 0, width: "100%", height: ALTO_FILA, transform: `translateY(${v.start}px)` }}
                    className={`grid ${COLUMNAS} items-center gap-2 border-b border-linea px-4 text-left ${
                      seleccionado ? "bg-linea/70" : "hover:bg-fondo"
                    } ${p.activo ? "" : "text-tenue line-through"}`}
                  >
                    <span className="truncate">{p.nombre}</span>
                    <span className="truncate text-tenue">
                      {p.categoriaId === null ? "—" : (nombreCategoria.get(p.categoriaId) ?? "—")}
                    </span>
                    <span className="truncate text-tenue">{p.codigoBarras ?? "—"}</span>
                    <span className="text-right">
                      {pesos(p.precioVenta)}
                      {p.unidad === "kg" && <span className="text-tenue">/kg</span>}
                    </span>
                    <span className={`text-right ${bajo || p.stockMil < 0 ? "font-semibold text-peligro" : ""}`}>
                      {cantidad(p.stockMil, p.unidad)}
                    </span>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </section>

      {panel && (
        <aside className="w-96 shrink-0 border-l border-linea bg-superficie">
          {panel.tipo === "categorias" ? (
            <PanelCategorias categorias={categorias} onCambio={cargarCategorias} onCerrar={() => setPanel(null)} />
          ) : (
            <FormularioProducto
              key={panel.tipo === "editar" ? panel.producto.id : `nuevo-${panel.codigo ?? ""}`}
              producto={panel.tipo === "editar" ? panel.producto : null}
              codigoInicial={panel.tipo === "nuevo" ? panel.codigo : undefined}
              categorias={categorias}
              categoriaInicial={typeof filtro === "number" ? filtro : null}
              onGuardado={alGuardar}
              onCerrar={() => setPanel(null)}
            />
          )}
        </aside>
      )}
    </div>
  );
}
