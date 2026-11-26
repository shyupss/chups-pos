import { Search } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { IMPRIMIBLE, useAtajos } from "../atajos";
import {
  api,
  textoError,
  type EstadoFiltro,
  type FiltrosVentas,
  type MedioPago,
  type TurnoBreve,
  type VentaListada,
} from "../api";
import DetalleVentaPanel from "../componentes/DetalleVentaPanel";
import InsigniaEstado from "../componentes/InsigniaEstado";
import Tabla, { type ColumnaTabla } from "../componentes/Tabla";
import { diaLocal, etiquetaDia, folio, hora, horaODia, NOMBRE_MEDIO, pesos } from "../formato";

interface Props {
  activa: boolean;
  /** Se intentó imprimir: conviene volver a leer el estado de la impresora. */
  onImpresion: () => void;
  /** Folio que se abre al llegar (enlace desde Inventario > Movimientos). */
  folioInicial?: number | null;
  onFolioUsado?: () => void;
}

const POR_PAGINA = 100;

type Periodo = "hoy" | "ayer" | "semana" | "mes" | "todas";
const PERIODOS: { id: Periodo; nombre: string }[] = [
  { id: "hoy", nombre: "Hoy" },
  { id: "ayer", nombre: "Ayer" },
  { id: "semana", nombre: "Últimos 7 días" },
  { id: "mes", nombre: "Este mes" },
  { id: "todas", nombre: "Todas" },
];

const ESTADOS: { id: EstadoFiltro; nombre: string }[] = [
  { id: "pagada", nombre: "Pagadas" },
  { id: "fiada", nombre: "Fiadas" },
  { id: "anulada", nombre: "Anuladas" },
  { id: "conDevolucion", nombre: "Con devolución" },
];

/** Días locales "aaaa-mm-dd" de cada atajo (hasta incluido). */
function rango(p: Periodo): { desde: string | null; hasta: string | null } {
  const hoy = new Date();
  const dia = (d: number) => diaLocal(new Date(hoy.getFullYear(), hoy.getMonth(), hoy.getDate() + d));
  switch (p) {
    case "hoy":
      return { desde: dia(0), hasta: dia(0) };
    case "ayer":
      return { desde: dia(-1), hasta: dia(-1) };
    case "semana":
      return { desde: dia(-6), hasta: dia(0) };
    case "mes":
      return { desde: diaLocal(new Date(hoy.getFullYear(), hoy.getMonth(), 1)), hasta: dia(0) };
    case "todas":
      return { desde: null, hasta: null };
  }
}

const periodoDe = (desde: string | null, hasta: string | null): Periodo | null =>
  PERIODOS.find((p) => {
    const r = rango(p.id);
    return r.desde === desde && r.hasta === hasta;
  })?.id ?? null;

const claseCampo = "h-11 rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento";

/** Ventas (F5): historial con filtros, detalle, reimpresión, anulación y devoluciones. */
export default function Ventas({ activa, onImpresion, folioInicial = null, onFolioUsado }: Props) {
  const [filtros, setFiltros] = useState<FiltrosVentas>(() => ({
    ...rango("hoy"),
    folio: null,
    medioPago: null,
    estado: null,
    turnoId: null,
  }));
  const [busqueda, setBusqueda] = useState("");
  const [filas, setFilas] = useState<VentaListada[]>([]);
  const [totales, setTotales] = useState<{ cantidad: number; total: number } | null>(null);
  const [hayMas, setHayMas] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [turnos, setTurnos] = useState<TurnoBreve[]>([]);
  const [abierta, setAbierta] = useState<number | null>(null);
  const cargando = useRef(false);
  const consultaActual = useRef(0);
  const buscador = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (folioInicial === null) return;
    api
      .ventaPorFolio(folioInicial)
      .then((id) => id !== null && setAbierta(id))
      .catch(() => {})
      .finally(() => onFolioUsado?.());
  }, [folioInicial, onFolioUsado]);

  useEffect(() => {
    api
      .listarTurnos(60, 0)
      .then(setTurnos)
      .catch(() => {});
  }, []);

  // El folio escrito filtra mientras se escribe (un folio exacto ignora los demás filtros).
  useEffect(() => {
    const t = setTimeout(() => {
      const n = Number(busqueda);
      setFiltros((f) => ({ ...f, folio: busqueda && n > 0 ? n : null }));
    }, 150);
    return () => clearTimeout(t);
  }, [busqueda]);

  const cargar = useCallback(
    async (desde: number) => {
      const consulta = desde === 0 ? ++consultaActual.current : consultaActual.current;
      cargando.current = true;
      try {
        const pagina = await api.listarVentas(filtros, POR_PAGINA, desde);
        if (consulta !== consultaActual.current) return; // llegó tarde
        setFilas((f) => (desde === 0 ? pagina.ventas : [...f, ...pagina.ventas]));
        if (pagina.totales) setTotales(pagina.totales);
        setHayMas(pagina.ventas.length === POR_PAGINA);
        setError(null);
      } catch (e) {
        setError(textoError(e));
      } finally {
        cargando.current = false;
      }
    },
    [filtros],
  );

  useEffect(() => {
    void cargar(0);
  }, [cargar]);

  const cargarMas = useCallback(() => {
    if (hayMas && !cargando.current && filas.length > 0) void cargar(filas.length);
  }, [hayMas, filas.length, cargar]);

  // Cada día lleva su encabezado (las ventas ya vienen ordenadas, de la más nueva a la más antigua).
  const grupoDia = useCallback((v: VentaListada) => ({ clave: diaLocal(new Date(v.fechaHora)), texto: etiquetaDia(v.fechaHora) }), []);

  // Hora/Folio · Total · Medio de pago · Productos · Estado.
  const columnas = useMemo<ColumnaTabla<VentaListada>[]>(
    () => [
      {
        id: "hora",
        titulo: "Hora y folio",
        ancho: "11rem",
        orden: (v) => v.fechaHora,
        celda: (v) => (
          <>
            <span className="font-medium">{hora(v.fechaHora)}</span> <span className="text-tenue">· {folio(v.folio)}</span>
          </>
        ),
      },
      {
        id: "total",
        titulo: "Total",
        ancho: "7.5rem",
        alinear: "derecha",
        orden: (v) => v.total,
        clase: (v) => (v.estado === "anulada" ? "line-through text-tenue" : "font-medium"),
        celda: (v) => pesos(v.total),
      },
      { id: "pago", titulo: "Medio de pago", ancho: "9rem", ocultarBajo: 640, orden: (v) => NOMBRE_MEDIO[v.medioPago], celda: (v) => NOMBRE_MEDIO[v.medioPago] },
      { id: "productos", titulo: "Productos", ancho: "6rem", alinear: "derecha", ocultarBajo: 760, orden: (v) => v.productos, celda: (v) => v.productos },
      { id: "estado", titulo: "Estado", ancho: "10.5rem", sinPuntos: true, orden: (v) => v.estado, celda: (v) => <InsigniaEstado estado={v.estado} /> },
    ],
    [],
  );

  /** Enter en el buscador: el folio escrito o escaneado (CODE128 de la boleta) abre esa venta. */
  const abrirPorFolio = async () => {
    const n = Number(busqueda);
    if (!busqueda || !(n > 0)) return;
    try {
      const id = await api.ventaPorFolio(n);
      if (id === null) setError(`No existe la venta ${folio(n)}`);
      else {
        setError(null);
        setAbierta(id);
      }
    } catch (e) {
      setError(textoError(e));
    }
    buscador.current?.select();
  };

  // Un dígito (o un folio escaneado) con el foco fuera de un campo va al buscador: lo enfoca y deja
  // que se escriba ahí (sin preventDefault). Con un diálogo abierto no corre.
  useAtajos(
    "pantalla",
    {
      [IMPRIMIBLE]: (e) => {
        if (/^\d$/.test(e.key)) buscador.current?.focus();
        return false;
      },
    },
    activa,
  );

  const periodo = periodoDe(filtros.desde, filtros.hasta);
  const cambiar = (parcial: Partial<FiltrosVentas>) => setFiltros((f) => ({ ...f, ...parcial }));
  const conFolio = filtros.folio !== null;

  /** Tras anular o devolver: la fila y los totales cambian. */
  const alCambiarVenta = () => void cargar(0);

  return (
    <div className="flex h-full">
      <section className="@container flex min-w-0 flex-1 flex-col p-6">
        <div className="flex flex-wrap items-center gap-3">
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void abrirPorFolio();
            }}
            className="relative min-w-52 flex-1"
          >
            <Search size={16} className="absolute top-3 left-3 text-tenue" aria-hidden />
            <input
              ref={buscador}
              autoFocus
              value={busqueda}
              onChange={(e) => setBusqueda(e.target.value.replace(/\D/g, "").slice(0, 9))}
              placeholder="Folio: escríbelo o escanea la boleta y Enter"
              inputMode="numeric"
              aria-label="Buscar por folio"
              className={`${claseCampo} w-full pl-9`}
            />
          </form>
          <div role="radiogroup" aria-label="Período" className="inline-flex rounded-lg border border-borde-control bg-suave p-0.5">
            {PERIODOS.map((p) => (
              <button
                key={p.id}
                type="button"
                role="radio"
                aria-checked={periodo === p.id}
                onClick={() => cambiar({ ...rango(p.id), turnoId: p.id === "todas" ? filtros.turnoId : null })}
                className={`h-8 rounded-md px-3 ${
                  periodo === p.id ? "bg-seleccion font-semibold text-acento shadow-s1" : "text-tenue hover:text-texto"
                }`}
              >
                {p.nombre}
              </button>
            ))}
          </div>
        </div>

        <div className="mt-3 flex flex-wrap items-center gap-3">
          <label className="flex items-center gap-2 text-tenue">
            Desde
            <input
              type="date"
              value={filtros.desde ?? ""}
              max={filtros.hasta ?? undefined}
              onChange={(e) => cambiar({ desde: e.target.value || null })}
              className={`${claseCampo} text-texto`}
            />
          </label>
          <label className="flex items-center gap-2 text-tenue">
            Hasta
            <input
              type="date"
              value={filtros.hasta ?? ""}
              min={filtros.desde ?? undefined}
              onChange={(e) => cambiar({ hasta: e.target.value || null })}
              className={`${claseCampo} text-texto`}
            />
          </label>
          <select
            value={filtros.medioPago ?? ""}
            onChange={(e) => cambiar({ medioPago: (e.target.value || null) as MedioPago | null })}
            aria-label="Medio de pago"
            className={claseCampo}
          >
            <option value="">Todos los medios</option>
            {(Object.keys(NOMBRE_MEDIO) as MedioPago[]).map((m) => (
              <option key={m} value={m}>
                {NOMBRE_MEDIO[m]}
              </option>
            ))}
          </select>
          <select
            value={filtros.estado ?? ""}
            onChange={(e) => cambiar({ estado: (e.target.value || null) as EstadoFiltro | null })}
            aria-label="Estado"
            className={claseCampo}
          >
            <option value="">Todos los estados</option>
            {ESTADOS.map((s) => (
              <option key={s.id} value={s.id}>
                {s.nombre}
              </option>
            ))}
          </select>
          <select
            value={filtros.turnoId ?? ""}
            // Un turno puede cruzar la medianoche: al elegirlo se miran todas las fechas.
            onChange={(e) => {
              const turnoId = e.target.value ? Number(e.target.value) : null;
              cambiar(turnoId === null ? { turnoId } : { turnoId, desde: null, hasta: null });
            }}
            aria-label="Turno"
            className={`${claseCampo} max-w-64`}
          >
            <option value="">Todos los turnos</option>
            {turnos.map((t) => (
              <option key={t.id} value={t.id}>
                Turno N° {t.id} · {horaODia(t.abiertoEn)}
                {t.abierto ? " (abierto)" : ""}
              </option>
            ))}
          </select>
        </div>

        {conFolio && (
          <p className="mt-3 rounded-lg bg-aviso-fondo px-3 py-2 text-aviso">
            Buscando el folio {folio(filtros.folio ?? 0)}: los demás filtros no se aplican.
          </p>
        )}
        {error && <p className="mt-3 rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}

        <Tabla
          etiqueta="Ventas"
          className="mt-4 flex-1"
          columnas={columnas}
          filas={filas}
          clave={(v) => v.id}
          onFila={(v) => setAbierta(v.id)}
          seleccionada={(v) => abierta === v.id}
          claseFila={(v) => (v.estado === "anulada" ? "text-tenue" : "")}
          grupo={grupoDia}
          reiniciar={JSON.stringify(filtros)}
          onCercaDelFinal={cargarMas}
          pie={
            <>
              <span>{totales === null ? "…" : totales.cantidad === 1 ? "1 venta" : `${totales.cantidad.toLocaleString("es-CL")} ventas`}</span>
              <span className="ml-auto" title="Sin las anuladas y descontando lo devuelto">
                Total <span className="text-base font-semibold text-texto">{totales === null ? "…" : pesos(totales.total)}</span>
              </span>
            </>
          }
          vacio={{ texto: conFolio ? "No hay una venta con ese folio." : "No hay ventas con estos filtros." }}
        />
      </section>

      {abierta !== null && (
        <aside className="w-[28rem] shrink-0 border-l border-linea bg-superficie">
          <DetalleVentaPanel
            key={abierta}
            ventaId={abierta}
            activa={activa}
            onImpresion={onImpresion}
            onCambio={alCambiarVenta}
            onCerrar={() => {
              setAbierta(null);
              buscador.current?.focus();
            }}
          />
        </aside>
      )}
    </div>
  );
}
