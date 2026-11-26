import Boton from "../componentes/Boton";
import BotonExportar from "../componentes/BotonExportar";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, textoError, type Categoria, type FiltroCategoria, type Producto } from "../api";
import FormularioProducto from "../componentes/FormularioProducto";
import Tabla, { type ColumnaTabla } from "../componentes/Tabla";
import { textoConteo } from "../componentes/tabla-nucleo";
import PanelCategorias from "../componentes/PanelCategorias";
import { avisarCambioStock } from "../eventos";
import { cantidad, pesos } from "../formato";

interface Props {
  codigoNuevo: string | null;
  onCodigoUsado: () => void;
  /** Abre Inventario > Movimientos filtrado por el producto. */
  onVerMovimientos: (p: { id: number; nombre: string }) => void;
}

const POR_PAGINA = 100;

type Panel =
  | { tipo: "nuevo"; codigo?: string }
  | { tipo: "editar"; producto: Producto }
  | { tipo: "categorias" }
  | null;

/** Valor del <select> de categoría: "todas", "sin" o el id como texto. */
const aFiltro = (valor: string): FiltroCategoria => (valor === "todas" || valor === "sin" ? valor : Number(valor));

export default function Productos({ codigoNuevo, onCodigoUsado, onVerMovimientos }: Props) {
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
    // Un texto vacío abre el formulario sin código (desde el asistente de primer arranque).
    if (codigoNuevo !== null) {
      setPanel({ tipo: "nuevo", codigo: codigoNuevo || undefined });
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
    void cargar(0);
  }, [cargar]);

  // Carga la siguiente página al acercarse al final.
  const cargarMas = useCallback(() => {
    if (hayMas && !cargando.current && filas.length > 0) void cargar(filas.length);
  }, [hayMas, filas.length, cargar]);

  // Nombre · Precio · Stock · Categoría · Código: lo que identifica, lo que más se mira, y lo secundario al final.
  const columnas = useMemo<ColumnaTabla<Producto>[]>(
    () => [
      { id: "nombre", titulo: "Nombre", ancho: "minmax(10rem,28rem)", orden: (p) => p.nombre, celda: (p) => <span title={p.nombre}>{p.nombre}</span> },
      {
        id: "precio",
        titulo: "Precio",
        ancho: "7.5rem",
        alinear: "derecha",
        orden: (p) => p.precioVenta,
        celda: (p) => (
          <>
            {pesos(p.precioVenta)}
            {p.unidad === "kg" && <span className="text-tenue">/kg</span>}
          </>
        ),
      },
      {
        id: "stock",
        titulo: "Stock",
        ancho: "6.5rem",
        alinear: "derecha",
        orden: (p) => p.stockMil,
        clase: (p) => (p.stockMinimoMil > 0 && p.stockMil <= p.stockMinimoMil) || p.stockMil < 0 ? "font-semibold text-peligro" : "",
        celda: (p) => cantidad(p.stockMil, p.unidad),
      },
      {
        id: "categoria",
        titulo: "Categoría",
        ancho: "9rem",
        ocultarBajo: 640,
        orden: (p) => (p.categoriaId === null ? null : nombreCategoria.get(p.categoriaId)),
        clase: () => "text-tenue",
        celda: (p) => (p.categoriaId === null ? "—" : (nombreCategoria.get(p.categoriaId) ?? "—")),
      },
      { id: "codigo", titulo: "Código", ancho: "9rem", ocultarBajo: 820, orden: (p) => p.codigoBarras, clase: () => "text-tenue", celda: (p) => p.codigoBarras ?? "—" },
    ],
    [nombreCategoria],
  );

  const alGuardar = (p: Producto) => {
    setFilas((f) => {
      const i = f.findIndex((x) => x.id === p.id);
      if (i === -1) return [p, ...f];
      const copia = f.slice();
      copia[i] = p;
      return copia;
    });
    setPanel({ tipo: "editar", producto: p });
    // Los contadores de productos por categoría pueden haber cambiado, y el mínimo o el stock inicial.
    void cargarCategorias();
    avisarCambioStock();
  };

  return (
    <div className="flex h-full">
      <section className="@container flex min-w-0 flex-1 flex-col p-6">
        {/* Los dos botones de acción van juntos y a la derecha; si la fila no alcanza, bajan juntos. */}
        <div className="flex flex-wrap items-center gap-3">
          <input
            value={texto}
            onChange={(e) => setTexto(e.target.value)}
            placeholder="Buscar por nombre o código"
            className="h-11 min-w-64 flex-1 rounded-lg border border-borde-control bg-superficie px-4 focus:border-acento"
            autoFocus
          />
          <select
            value={String(filtro)}
            onChange={(e) => setFiltro(aFiltro(e.target.value))}
            aria-label="Filtrar por categoría"
            className="h-11 max-w-48 rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento"
          >
            <option value="todas">Todas las categorías</option>
            {categorias.map((c) => (
              <option key={c.id} value={c.id}>
                {c.nombre}
              </option>
            ))}
            <option value="sin">Sin categoría</option>
          </select>
          <Boton onClick={() => setPanel({ tipo: "categorias" })}>Categorías</Boton>
          <label className="flex items-center gap-2 text-tenue">
            <input type="checkbox" checked={inactivos} onChange={(e) => setInactivos(e.target.checked)} />
            Ver desactivados
          </label>
          <div className="ml-auto flex flex-wrap items-center justify-end gap-3">
            <BotonExportar que="productos" />
            <Boton variante="primario" onClick={() => setPanel({ tipo: "nuevo" })}>
              Nuevo producto
            </Boton>
          </div>
        </div>

        {error && <p className="mt-3 rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}

        <Tabla
          etiqueta="Productos"
          className="mt-4 flex-1"
          columnas={columnas}
          filas={filas}
          clave={(p) => p.id}
          onFila={(p) => setPanel({ tipo: "editar", producto: p })}
          seleccionada={(p) => panel?.tipo === "editar" && panel.producto.id === p.id}
          claseFila={(p) => (p.activo ? "" : "text-tenue line-through")}
          reiniciar={`${busqueda}|${inactivos}|${String(filtro)}`}
          onCercaDelFinal={cargarMas}
          pie={textoConteo(filas.length, "producto", "productos", hayMas)}
          vacio={
            busqueda || filtro !== "todas"
              ? { texto: "No hay productos con esa búsqueda." }
              : { texto: "Todavía no hay productos.", accion: { etiqueta: "Nuevo producto", onClick: () => setPanel({ tipo: "nuevo" }) } }
          }
        />
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
              onVerMovimientos={onVerMovimientos}
              onCerrar={() => setPanel(null)}
            />
          )}
        </aside>
      )}
    </div>
  );
}
