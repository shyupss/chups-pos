import { useVirtualizer } from "@tanstack/react-virtual";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { api, textoError, type Cliente, type Config, type DatosCliente, type Deudor } from "../api";
import { aEntero, fechaHora, pesos } from "../formato";
import FichaCliente, { diasDesde } from "./fiados/FichaCliente";

const POR_PAGINA = 100;
const ALTO_FILA = 60;
const claseCampo = "h-10 w-full rounded-lg border border-linea bg-fondo px-3 outline-none focus:border-texto";

type Panel = { tipo: "ficha"; id: number } | { tipo: "nuevo" } | { tipo: "editar"; cliente: Cliente } | null;

interface Props {
  activa: boolean;
  onIrAVenta: (folio: number) => void;
  onImpresion: () => void;
}

/** Fiados (F4): quién debe y cuánto, la ficha de cada cliente y sus pagos. */
export default function Fiados({ activa, onIrAVenta, onImpresion }: Props) {
  const [texto, setTexto] = useState("");
  const [busqueda, setBusqueda] = useState("");
  const [todos, setTodos] = useState(false);
  const [filas, setFilas] = useState<Deudor[]>([]);
  const [hayMas, setHayMas] = useState(true);
  const [total, setTotal] = useState<{ monto: number; clientes: number } | null>(null);
  const [panel, setPanel] = useState<Panel>(null);
  const [config, setConfig] = useState<Config | null>(null);
  const [error, setError] = useState<string | null>(null);
  const cargando = useRef(false);
  const consultaActual = useRef(0);
  const contenedor = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (activa) api.obtenerConfig().then(setConfig).catch(() => {});
  }, [activa]);

  useEffect(() => {
    const t = setTimeout(() => setBusqueda(texto), 150);
    return () => clearTimeout(t);
  }, [texto]);

  const cargar = useCallback(
    async (desde: number) => {
      const consulta = desde === 0 ? ++consultaActual.current : consultaActual.current;
      cargando.current = true;
      try {
        const p = await api.listarDeudores(busqueda, todos, POR_PAGINA, desde);
        if (consulta !== consultaActual.current) return;
        setFilas((f) => (desde === 0 ? p.clientes : [...f, ...p.clientes]));
        setHayMas(p.clientes.length === POR_PAGINA);
        if (p.totalAdeudado !== null) setTotal({ monto: p.totalAdeudado, clientes: p.clientesConDeuda ?? 0 });
        setError(null);
      } catch (e) {
        setError(textoError(e));
      } finally {
        cargando.current = false;
      }
    },
    [busqueda, todos],
  );
  useEffect(() => {
    if (activa) void cargar(0);
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
    if (hayMas && !cargando.current && ultimaVisible >= filas.length - 20 && filas.length > 0) void cargar(filas.length);
  }, [ultimaVisible, filas.length, hayMas, cargar]);

  const diasAlerta = config?.fiadoDiasAlerta ?? 30;
  const refrescar = useCallback(() => void cargar(0), [cargar]);

  return (
    <div className="flex h-full gap-4 p-6">
      <section className="flex w-[26rem] shrink-0 flex-col">
        <div className="flex items-baseline justify-between">
          <h1 className="text-xl font-semibold">Fiados</h1>
          {total && (
            <span className="text-tenue tabular-nums">
              Por cobrar <span className="font-semibold text-texto">{pesos(total.monto)}</span> ·{" "}
              {total.clientes === 1 ? "1 cliente" : `${total.clientes} clientes`}
            </span>
          )}
        </div>
        <div className="mt-3 flex gap-2">
          <input
            value={texto}
            onChange={(e) => setTexto(e.target.value)}
            placeholder="Buscar por nombre o teléfono"
            aria-label="Buscar cliente"
            className="h-10 min-w-0 flex-1 rounded-lg border border-linea bg-superficie px-4 outline-none focus:border-texto"
          />
          <button
            type="button"
            onClick={() => setPanel({ tipo: "nuevo" })}
            className="h-10 shrink-0 rounded-lg bg-acento px-4 font-semibold text-sobre-acento"
          >
            Nuevo cliente
          </button>
        </div>
        <label className="mt-2 flex items-center gap-2 text-tenue">
          <input type="checkbox" checked={todos} onChange={(e) => setTodos(e.target.checked)} />
          Ver también a los que no deben
        </label>
        {error && <p className="mt-3 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}
        <div ref={contenedor} className="mt-3 min-h-0 flex-1 overflow-auto rounded-lg border border-linea bg-superficie">
          {filas.length === 0 ? (
            <p className="p-8 text-center text-tenue">
              {busqueda ? "Sin resultados" : todos ? "Todavía no hay clientes." : "Nadie debe nada."}
            </p>
          ) : (
            <div style={{ height: virtual.getTotalSize(), position: "relative" }}>
              {visibles.map((v) => {
                const d = filas[v.index];
                const elegido = panel?.tipo === "ficha" && panel.id === d.id;
                const dias = d.fiadaMasAntigua ? diasDesde(d.fiadaMasAntigua) : null;
                const atrasado = dias !== null && dias >= diasAlerta;
                return (
                  <button
                    key={d.id}
                    onClick={() => setPanel({ tipo: "ficha", id: d.id })}
                    style={{ position: "absolute", top: 0, left: 0, width: "100%", height: ALTO_FILA, transform: `translateY(${v.start}px)` }}
                    className={`flex items-center gap-3 border-b border-linea px-4 text-left ${elegido ? "bg-linea/70" : "hover:bg-fondo"} ${
                      d.activo ? "" : "text-tenue"
                    }`}
                  >
                    <span className="min-w-0 flex-1">
                      <span className="block truncate font-medium">{d.nombre}</span>
                      <span className="block truncate text-xs text-tenue">
                        {d.ultimoPago ? `Último pago ${fechaHora(d.ultimoPago)}` : "Sin pagos"}
                        {d.telefono && ` · ${d.telefono}`}
                      </span>
                    </span>
                    <span className="shrink-0 text-right tabular-nums">
                      <span className="block font-semibold">{d.deuda > 0 ? pesos(d.deuda) : "Al día"}</span>
                      {dias !== null && (
                        <span className={`text-xs ${atrasado ? "rounded bg-aviso-fondo px-1.5 font-medium text-aviso" : "text-tenue"}`}>
                          {dias === 0 ? "desde hoy" : `hace ${dias} ${dias === 1 ? "día" : "días"}`}
                        </span>
                      )}
                    </span>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </section>

      <section className="min-w-0 flex-1 rounded-lg border border-linea bg-superficie">
        {panel === null && (
          <p className="p-8 text-center text-tenue">Elige un cliente para ver su cuenta y registrar pagos.</p>
        )}
        {panel?.tipo === "ficha" && (
          <FichaCliente
            key={panel.id}
            clienteId={panel.id}
            activa={activa}
            config={config}
            onCambio={refrescar}
            onEditar={() => {
              api
                .obtenerCliente(panel.id)
                .then((c) => setPanel({ tipo: "editar", cliente: c }))
                .catch((e) => setError(textoError(e)));
            }}
            onIrAVenta={onIrAVenta}
            onImpresion={onImpresion}
          />
        )}
        {(panel?.tipo === "nuevo" || panel?.tipo === "editar") && (
          <FormularioCliente
            key={panel.tipo === "editar" ? panel.cliente.id : "nuevo"}
            cliente={panel.tipo === "editar" ? panel.cliente : null}
            onGuardado={(c) => {
              setPanel({ tipo: "ficha", id: c.id });
              refrescar();
            }}
            onCerrar={() => setPanel(panel.tipo === "editar" ? { tipo: "ficha", id: panel.cliente.id } : null)}
          />
        )}
      </section>
    </div>
  );
}

function FormularioCliente({
  cliente,
  onGuardado,
  onCerrar,
}: {
  cliente: Cliente | null;
  onGuardado: (c: Cliente) => void;
  onCerrar: () => void;
}) {
  const [datos, setDatos] = useState({
    nombre: cliente?.nombre ?? "",
    telefono: cliente?.telefono ?? "",
    rut: cliente?.rut ?? "",
    direccion: cliente?.direccion ?? "",
    nota: cliente?.nota ?? "",
    limite: cliente?.limiteCredito === null || cliente === null ? "" : String(cliente.limiteCredito),
  });
  const [error, setError] = useState<string | null>(null);
  const [guardando, setGuardando] = useState(false);
  const cambiar = (campo: keyof typeof datos) => (e: { target: { value: string } }) =>
    setDatos((d) => ({ ...d, [campo]: e.target.value }));

  const guardar = async () => {
    setGuardando(true);
    setError(null);
    const limite = datos.limite.trim() === "" ? null : aEntero(datos.limite);
    const enviar: DatosCliente = {
      nombre: datos.nombre,
      telefono: datos.telefono || null,
      rut: datos.rut || null,
      direccion: datos.direccion || null,
      nota: datos.nota || null,
      limiteCredito: limite,
    };
    try {
      onGuardado(cliente ? await api.editarCliente(cliente.id, enviar) : await api.crearCliente(enviar));
    } catch (e) {
      setError(textoError(e));
    } finally {
      setGuardando(false);
    }
  };

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        void guardar();
      }}
      onKeyDown={(e) => e.key === "Escape" && onCerrar()}
      className="flex max-w-lg flex-col gap-3 p-5"
    >
      <h2 className="text-lg font-semibold">{cliente ? `Editar ${cliente.nombre}` : "Nuevo cliente"}</h2>
      <Campo etiqueta="Nombre (obligatorio)">
        <input autoFocus value={datos.nombre} onChange={cambiar("nombre")} maxLength={80} className={claseCampo} />
      </Campo>
      <div className="grid grid-cols-2 gap-3">
        <Campo etiqueta="Teléfono (opcional)">
          <input value={datos.telefono} onChange={cambiar("telefono")} maxLength={30} className={claseCampo} />
        </Campo>
        <Campo etiqueta="RUT (opcional)">
          <input value={datos.rut} onChange={cambiar("rut")} placeholder="12.345.678-5" maxLength={16} className={claseCampo} />
        </Campo>
      </div>
      <Campo etiqueta="Dirección (opcional)">
        <input value={datos.direccion} onChange={cambiar("direccion")} maxLength={120} className={claseCampo} />
      </Campo>
      <Campo etiqueta="Límite de crédito (vacío = sin límite)">
        <input
          value={datos.limite ? pesos(aEntero(datos.limite) ?? 0) : ""}
          onChange={cambiar("limite")}
          inputMode="numeric"
          placeholder="Sin límite"
          className={`${claseCampo} tabular-nums`}
        />
      </Campo>
      <Campo etiqueta="Nota (opcional)">
        <input value={datos.nota} onChange={cambiar("nota")} maxLength={200} className={claseCampo} />
      </Campo>
      {error && <p className="rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}
      <div className="mt-2 flex gap-2">
        <button type="button" onClick={onCerrar} className="h-10 flex-1 rounded-lg text-tenue hover:bg-fondo hover:text-texto">
          Cancelar <span className="text-xs opacity-60">Esc</span>
        </button>
        <button
          type="submit"
          disabled={guardando || !datos.nombre.trim()}
          className="h-10 flex-1 rounded-lg bg-acento font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
        >
          {cliente ? "Guardar" : "Crear cliente"}
        </button>
      </div>
    </form>
  );
}

function Campo({ etiqueta, children }: { etiqueta: string; children: ReactNode }) {
  return (
    <label className="block">
      <span className="text-tenue">{etiqueta}</span>
      <div className="mt-1">{children}</div>
    </label>
  );
}
