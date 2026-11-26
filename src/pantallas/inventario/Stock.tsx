import Boton from "../../componentes/Boton";
import BotonExportar from "../../componentes/BotonExportar";
import { ClipboardList, History, PackageMinus, Scale } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
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
import Insignia from "../../componentes/Insignia";
import Opciones, { type Opcion } from "../../componentes/Opciones";
import Tabla, { type ColumnaTabla } from "../../componentes/Tabla";
import { textoConteo } from "../../componentes/tabla-nucleo";
import { avisarCambioStock } from "../../eventos";
import { cantidad, pesos } from "../../formato";

const POR_PAGINA = 100;

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
    void cargar(0);
    leerResumen();
  }, [activa, cargar, leerResumen]);

  const cargarMas = useCallback(() => {
    if (hayMas && !cargando.current && filas.length > 0) void cargar(filas.length);
  }, [hayMas, filas.length, cargar]);

  // Nombre · Stock · Mínimo · Estado · Costo · Valor.
  const columnas = useMemo<ColumnaTabla<ProductoStock>[]>(
    () => [
      { id: "nombre", titulo: "Nombre", ancho: "minmax(10rem,28rem)", orden: (p) => p.nombre, celda: (p) => <span title={p.nombre}>{p.nombre}</span> },
      {
        id: "stock",
        titulo: "Stock",
        ancho: "6.5rem",
        alinear: "derecha",
        orden: (p) => p.stockMil,
        clase: (p) => (p.stockMil < 0 || p.bajoMinimo ? "font-semibold text-peligro" : p.stockMil === 0 ? "text-tenue" : ""),
        celda: (p) => cantidad(p.stockMil, p.unidad),
      },
      {
        id: "minimo",
        titulo: "Mínimo",
        ancho: "6.5rem",
        alinear: "derecha",
        ocultarBajo: 620,
        orden: (p) => p.stockMinimoMil,
        clase: () => "text-tenue",
        celda: (p) => (p.stockMinimoMil > 0 ? cantidad(p.stockMinimoMil, p.unidad) : "—"),
      },
      {
        id: "estado",
        titulo: "Estado",
        ancho: "9.5rem",
        sinPuntos: true,
        orden: (p) => (p.stockMil < 0 ? 0 : p.bajoMinimo ? 1 : p.stockMil === 0 ? 2 : 3),
        celda: (p) =>
          p.stockMil < 0 ? (
            <Insignia tono="peligro">Negativo</Insignia>
          ) : p.bajoMinimo ? (
            <Insignia tono="aviso">Bajo el mínimo</Insignia>
          ) : p.stockMil === 0 ? (
            <Insignia tono="neutro">Sin stock</Insignia>
          ) : (
            <span className="text-tenue">—</span>
          ),
      },
      {
        id: "costo",
        titulo: "Costo",
        ancho: "7rem",
        alinear: "derecha",
        ocultarBajo: 820,
        orden: (p) => p.costo,
        clase: () => "text-tenue",
        celda: (p) => (p.costo === null ? "—" : `${pesos(p.costo)}${p.unidad === "kg" ? "/kg" : ""}`),
      },
      {
        id: "valor",
        titulo: "Valor",
        ancho: "7.5rem",
        alinear: "derecha",
        sinPuntos: true,
        orden: (p) => p.valor,
        celda: (p) => (p.valor === null ? <Insignia tono="aviso">Sin costo</Insignia> : pesos(p.valor)),
      },
    ],
    [],
  );

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

        <Tabla
          etiqueta="Stock"
          className="mt-4 flex-1"
          columnas={columnas}
          filas={filas}
          clave={(p) => p.id}
          onFila={setElegido}
          seleccionada={(p) => elegido?.id === p.id}
          reiniciar={`${busqueda}|${categoria}|${estado}`}
          onCercaDelFinal={cargarMas}
          pie={textoConteo(filas.length, "producto", "productos", hayMas)}
          vacio={{ texto: "No hay productos con estos filtros." }}
        />
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
