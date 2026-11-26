import Boton from "../../componentes/Boton";
import BotonExportar from "../../componentes/BotonExportar";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ClipboardList, History, PackageMinus, Scale } from "lucide-react";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import {
  api,
  textoError,
  type Categoria,
  type FiltroEstadoStock,
  type ProductoStock,
  type ResumenInventario,
} from "../../api";
import ConteoInventario from "../../componentes/ConteoInventario";
import DialogoAjuste from "../../componentes/DialogoAjuste";
import DialogoMerma from "../../componentes/DialogoMerma";
import Opciones, { type Opcion } from "../../componentes/Opciones";
import { avisarCambioStock } from "../../eventos";
import { cantidad, pesos } from "../../formato";

const POR_PAGINA = 100;
const ALTO_FILA = 40;
const COLUMNAS =
  "grid-cols-[minmax(0,1fr)_6.5rem_6.5rem] @lg:grid-cols-[minmax(0,1fr)_6.5rem_6rem_6.5rem_7rem] @2xl:grid-cols-[minmax(0,1fr)_7rem_6.5rem_6.5rem_7.5rem]";
const COL_ANCHA = "hidden @lg:block";

const ESTADOS: Opcion<FiltroEstadoStock>[] = [
  { id: "todos", nombre: "Todos" },
  { id: "bajoMinimo", nombre: "Bajo el mínimo" },
  { id: "sinStock", nombre: "Sin stock" },
  { id: "negativo", nombre: "Negativo" },
];

type Dialogo = "ajuste" | "merma" | "conteo" | null;

interface Props {
  activa: boolean;
  onVerMovimientos: (p: { id: number; nombre: string }) => void;
  estadoInicial?: FiltroEstadoStock;
}

/** Inventario > Stock: stock actual, mínimo, costo y valor, con totales y acciones por producto. */
export default function Stock({ activa, onVerMovimientos, estadoInicial }: Props) {
  const [texto, setTexto] = useState("");
  const [busqueda, setBusqueda] = useState("");
  const [categoria, setCategoria] = useState<string>("todas");
  const [estado, setEstado] = useState<FiltroEstadoStock>(estadoInicial ?? "todos");
  const [categorias, setCategorias] = useState<Categoria[]>([]);
  const [filas, setFilas] = useState<ProductoStock[]>([]);
  const [hayMas, setHayMas] = useState(true);
  const [resumen, setResumen] = useState<ResumenInventario | null>(null);
  const [elegido, setElegido] = useState<ProductoStock | null>(null);
  const [dialogo, setDialogo] = useState<Dialogo>(null);
  const [aviso, setAviso] = useState<{ error: boolean; texto: string } | null>(null);
  const cargando = useRef(false);
  const consultaActual = useRef(0);
  const contenedor = useRef<HTMLDivElement>(null);

  useEffect(() => {
    api.listarCategorias().then(setCategorias).catch(() => {});
  }, []);
  useEffect(() => {
    const t = setTimeout(() => setBusqueda(texto), 150);
    return () => clearTimeout(t);
  }, [texto]);

  const leerResumen = useCallback(() => {
    api.resumenInventario().then(setResumen).catch(() => {});
  }, []);

  const cargar = useCallback(
    async (desde: number) => {
      const consulta = desde === 0 ? ++consultaActual.current : consultaActual.current;
      cargando.current = true;
      try {
        const pagina = await api.listarStock(
          {
            texto: busqueda,
            categoriaId: categoria === "todas" || categoria === "sin" ? null : Number(categoria),
            sinCategoria: categoria === "sin",
            estado,
          },
          POR_PAGINA,
          desde,
        );
        if (consulta !== consultaActual.current) return;
        setFilas((f) => (desde === 0 ? pagina : [...f, ...pagina]));
        setHayMas(pagina.length === POR_PAGINA);
      } catch (e) {
        setAviso({ error: true, texto: textoError(e) });
      } finally {
        cargando.current = false;
      }
    },
    [busqueda, categoria, estado],
  );

  useEffect(() => {
    if (!activa) return;
    contenedor.current?.scrollTo({ top: 0 });
    void cargar(0);
    leerResumen();
  }, [activa, cargar, leerResumen]);

  const virtual = useVirtualizer({
    count: filas.length,
    getScrollElement: () => contenedor.current,
    estimateSize: () => ALTO_FILA,
    overscan: 10,
  });
  const visibles = virtual.getVirtualItems();
  const ultimaVisible = visibles.at(-1)?.index ?? 0;
  useEffect(() => {
    if (hayMas && !cargando.current && ultimaVisible >= filas.length - 20 && filas.length > 0) void cargar(filas.length);
  }, [ultimaVisible, filas.length, hayMas, cargar]);

  /** Tras un ajuste o una merma: lista, totales, insignia y el producto elegido al día. */
  const alCambiarStock = async (texto: string) => {
    setDialogo(null);
    setAviso({ error: false, texto });
    avisarCambioStock();
    leerResumen();
    await cargar(0);
    if (elegido) {
      const [p] = await api.listarStock(
        { texto: elegido.codigoBarras ?? elegido.nombre, categoriaId: null, sinCategoria: false, estado: "todos" },
        20,
        0,
      );
      if (p && p.id === elegido.id) setElegido(p);
    }
  };

  return (
    <div className="flex min-h-0 flex-1">
      <section className="@container flex min-w-0 flex-1 flex-col">
        <div className="grid grid-cols-2 gap-3 @3xl:grid-cols-4">
          <Total titulo="Valor a costo" valor={resumen ? pesos(resumen.valorCosto) : "…"}>
            {resumen && resumen.sinCosto > 0 && (
              <span className="text-aviso">{resumen.sinCosto === 1 ? "1 producto sin costo" : `${resumen.sinCosto} productos sin costo`} (cuentan $0)</span>
            )}
          </Total>
          <Total titulo="Valor a precio de venta" valor={resumen ? pesos(resumen.valorVenta) : "…"} />
          <Total titulo="Bajo el mínimo" valor={resumen ? String(resumen.bajoMinimo) : "…"} alerta={(resumen?.bajoMinimo ?? 0) > 0} />
          <Total titulo="Productos activos" valor={resumen ? String(resumen.productos) : "…"} />
        </div>

        <div className="mt-4 flex flex-wrap items-center gap-3">
          <input
            value={texto}
            onChange={(e) => setTexto(e.target.value)}
            placeholder="Buscar por nombre o código"
            aria-label="Buscar producto"
            className="h-11 min-w-64 flex-1 rounded-lg border border-borde-control bg-superficie px-4 focus:border-acento"
          />
          <select
            value={categoria}
            onChange={(e) => setCategoria(e.target.value)}
            aria-label="Categoría"
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
          <Opciones nombre="estado-stock" etiqueta="Estado del stock" opciones={ESTADOS} valor={estado} onCambiar={setEstado} />
          <Boton icono={ClipboardList} onClick={() => setDialogo("conteo")}>
            Contar inventario
          </Boton>
          <BotonExportar que="stock" etiqueta="Excel" />
        </div>

        {aviso && (
          <p
            role="status"
            className={`mt-3 rounded-lg px-3 py-2 ${aviso.error ? "bg-peligro-fondo text-peligro" : "bg-exito-fondo text-exito"}`}
          >
            {aviso.texto}
          </p>
        )}

        <div className={`mt-4 grid ${COLUMNAS} gap-2 px-4 text-xs font-medium text-tenue`}>
          <span>Producto</span>
          <span className="text-right">Stock</span>
          <span className={`${COL_ANCHA} text-right`}>Mínimo</span>
          <span className={`${COL_ANCHA} text-right`}>Costo</span>
          <span className="text-right">Valor</span>
        </div>
        <div ref={contenedor} className="mt-1 min-h-0 flex-1 overflow-auto tarjeta tabular-nums">
          {filas.length === 0 ? (
            <p className="p-8 text-center text-tenue">No hay productos con estos filtros.</p>
          ) : (
            <div style={{ height: virtual.getTotalSize(), position: "relative" }}>
              {visibles.map((v) => {
                const p = filas[v.index];
                return (
                  <button
                    key={p.id}
                    onClick={() => setElegido(p)}
                    aria-current={elegido?.id === p.id ? "true" : undefined}
                    style={{ position: "absolute", top: 0, left: 0, width: "100%", height: ALTO_FILA, transform: `translateY(${v.start}px)` }}
                    className={`grid ${COLUMNAS} items-center gap-2 border-b border-linea px-4 text-left ${
                      elegido?.id === p.id ? "bg-seleccion" : "hover:bg-suave"
                    }`}
                  >
                    <span className="flex min-w-0 items-center gap-2">
                      <span className="truncate" title={p.nombre}>
                        {p.nombre}
                      </span>
                      {p.bajoMinimo && p.stockMil >= 0 && (
                        <span className="shrink-0 rounded-full bg-aviso-fondo px-2 text-xs leading-5 font-medium text-aviso">
                          Bajo el mínimo
                        </span>
                      )}
                    </span>
                    <span
                      className={`text-right ${p.stockMil < 0 || p.bajoMinimo ? "font-semibold text-peligro" : ""} ${
                        p.stockMil === 0 ? "text-tenue" : ""
                      }`}
                    >
                      {cantidad(p.stockMil, p.unidad)}
                    </span>
                    <span className={`${COL_ANCHA} text-right text-tenue`}>
                      {p.stockMinimoMil > 0 ? cantidad(p.stockMinimoMil, p.unidad) : "—"}
                    </span>
                    <span className={`${COL_ANCHA} text-right text-tenue`}>
                      {p.costo === null ? "—" : `${pesos(p.costo)}${p.unidad === "kg" ? "/kg" : ""}`}
                    </span>
                    <span className="text-right">{p.valor === null ? <span className="text-aviso">sin costo</span> : pesos(p.valor)}</span>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </section>

      {elegido && (
        <aside className="ml-4 w-80 shrink-0 tarjeta p-5">
          <div className="flex items-start justify-between gap-2">
            <h2 className="font-semibold">{elegido.nombre}</h2>
            <Boton tamano="icono" onClick={() => setElegido(null)} aria-label="Cerrar">
              ×
            </Boton>
          </div>
          {elegido.codigoBarras && <div className="text-xs text-tenue">{elegido.codigoBarras}</div>}
          <dl className="mt-4 space-y-1 tabular-nums">
            <Par etiqueta="Stock" valor={cantidad(elegido.stockMil, elegido.unidad)} fuerte />
            <Par etiqueta="Mínimo" valor={elegido.stockMinimoMil > 0 ? cantidad(elegido.stockMinimoMil, elegido.unidad) : "—"} />
            <Par etiqueta="Costo" valor={elegido.costo === null ? "sin costo" : pesos(elegido.costo)} />
            <Par etiqueta="Precio" valor={pesos(elegido.precioVenta)} />
            <Par etiqueta="Valor a costo" valor={elegido.valor === null ? "—" : pesos(elegido.valor)} />
          </dl>
          <div className="mt-5 space-y-2">
            <Accion onClick={() => setDialogo("ajuste")} icono={<Scale size={16} aria-hidden />}>
              Ajustar stock (conteo)
            </Accion>
            <Accion onClick={() => setDialogo("merma")} icono={<PackageMinus size={16} aria-hidden />}>
              Registrar merma
            </Accion>
            <Accion onClick={() => onVerMovimientos(elegido)} icono={<History size={16} aria-hidden />}>
              Ver movimientos
            </Accion>
          </div>
        </aside>
      )}

      {dialogo === "conteo" && (
        <ConteoInventario
          onCerrar={() => setDialogo(null)}
          onTerminado={(d) => {
            const cambiados = d.filter((x) => x.diferenciaMil !== 0).length;
            void alCambiarStock(`Inventario ajustado: ${cambiados} ${cambiados === 1 ? "producto" : "productos"} con diferencia.`);
          }}
        />
      )}
      {dialogo === "ajuste" && elegido && (
        <DialogoAjuste
          producto={elegido}
          onCerrar={() => setDialogo(null)}
          onAjustado={(d) => void alCambiarStock(`Stock de ${d.nombre} ajustado a ${cantidad(d.contadoMil, d.unidad)}.`)}
        />
      )}
      {dialogo === "merma" && elegido && (
        <DialogoMerma
          producto={elegido}
          onCerrar={() => setDialogo(null)}
          onRegistrada={(s) => void alCambiarStock(`Merma registrada: a ${elegido.nombre} le quedan ${cantidad(s, elegido.unidad)}.`)}
        />
      )}
    </div>
  );
}

function Total({ titulo, valor, alerta, children }: { titulo: string; valor: string; alerta?: boolean; children?: ReactNode }) {
  return (
    <div className="tarjeta px-4 py-3">
      <div className="text-xs font-medium text-tenue">{titulo}</div>
      <div className={`text-2xl font-semibold tabular-nums ${alerta ? "text-peligro" : ""}`}>{valor}</div>
      {children && <div className="text-xs">{children}</div>}
    </div>
  );
}

function Par({ etiqueta, valor, fuerte }: { etiqueta: string; valor: string; fuerte?: boolean }) {
  return (
    <div className={`flex justify-between gap-3 ${fuerte ? "text-lg font-semibold" : ""}`}>
      <dt className={fuerte ? "" : "text-tenue"}>{etiqueta}</dt>
      <dd>{valor}</dd>
    </div>
  );
}

function Accion({ onClick, icono, children }: { onClick: () => void; icono: ReactNode; children: ReactNode }) {
  return (
    <Boton bloque onClick={onClick}>
      <span className="text-tenue">{icono}</span>
      {children}
    </Boton>
  );
}
