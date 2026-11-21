import { useVirtualizer } from "@tanstack/react-virtual";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { api, textoError, type DatosProveedor, type Proveedor } from "../../api";
import { fechaHora } from "../../formato";

const POR_PAGINA = 100;
const ALTO_FILA = 44;
const VACIO: DatosProveedor = { nombre: "", rut: "", telefono: "", contacto: "", nota: "" };
const claseCampo = "h-10 w-full rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento";

type Panel = { tipo: "nuevo" } | { tipo: "editar"; proveedor: Proveedor } | null;

/** Inventario > Proveedores: buscar, crear, editar y desactivar (nunca se borran). */
export default function Proveedores({ activa }: { activa: boolean }) {
  const [texto, setTexto] = useState("");
  const [busqueda, setBusqueda] = useState("");
  const [inactivos, setInactivos] = useState(false);
  const [filas, setFilas] = useState<Proveedor[]>([]);
  const [hayMas, setHayMas] = useState(true);
  const [panel, setPanel] = useState<Panel>(null);
  const [error, setError] = useState<string | null>(null);
  const cargando = useRef(false);
  const consultaActual = useRef(0);
  const contenedor = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const t = setTimeout(() => setBusqueda(texto), 150);
    return () => clearTimeout(t);
  }, [texto]);

  const cargar = useCallback(
    async (desde: number) => {
      const consulta = desde === 0 ? ++consultaActual.current : consultaActual.current;
      cargando.current = true;
      try {
        const pagina = await api.listarProveedores(busqueda, inactivos, POR_PAGINA, desde);
        if (consulta !== consultaActual.current) return;
        setFilas((f) => (desde === 0 ? pagina : [...f, ...pagina]));
        setHayMas(pagina.length === POR_PAGINA);
        setError(null);
      } catch (e) {
        setError(textoError(e));
      } finally {
        cargando.current = false;
      }
    },
    [busqueda, inactivos],
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

  const alGuardar = (p: Proveedor) => {
    setPanel({ tipo: "editar", proveedor: p });
    void cargar(0);
  };

  return (
    <div className="flex min-h-0 flex-1">
      <section className="flex min-w-0 flex-1 flex-col">
        <div className="flex flex-wrap items-center gap-3">
          <input
            value={texto}
            onChange={(e) => setTexto(e.target.value)}
            placeholder="Buscar por nombre, contacto o RUT"
            aria-label="Buscar proveedor"
            className="h-10 min-w-48 flex-1 rounded-lg border border-borde-control bg-superficie px-4 focus:border-acento"
          />
          <label className="flex items-center gap-2 text-tenue">
            <input type="checkbox" checked={inactivos} onChange={(e) => setInactivos(e.target.checked)} />
            Ver desactivados
          </label>
          <button
            type="button"
            onClick={() => setPanel({ tipo: "nuevo" })}
            className="h-10 rounded-lg bg-acento px-4 font-semibold text-sobre-acento"
          >
            Nuevo proveedor
          </button>
        </div>
        {error && <p className="mt-3 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}
        <div ref={contenedor} className="mt-4 min-h-0 flex-1 overflow-auto tarjeta">
          {filas.length === 0 ? (
            <p className="p-8 text-center text-tenue">
              {busqueda ? "Sin resultados" : 'Todavía no hay proveedores. Crea el primero con "Nuevo proveedor".'}
            </p>
          ) : (
            <div style={{ height: virtual.getTotalSize(), position: "relative" }}>
              {visibles.map((v) => {
                const p = filas[v.index];
                const elegido = panel?.tipo === "editar" && panel.proveedor.id === p.id;
                return (
                  <button
                    key={p.id}
                    onClick={() => setPanel({ tipo: "editar", proveedor: p })}
                    style={{ position: "absolute", top: 0, left: 0, width: "100%", height: ALTO_FILA, transform: `translateY(${v.start}px)` }}
                    className={`flex items-center gap-3 border-b border-linea px-4 text-left ${
                      elegido ? "bg-linea/70" : "hover:bg-fondo"
                    } ${p.activo ? "" : "text-tenue line-through"}`}
                  >
                    <span className="min-w-0 flex-1 truncate">
                      {p.nombre}
                      {p.contacto && <span className="text-tenue"> · {p.contacto}</span>}
                    </span>
                    <span className="text-xs text-tenue tabular-nums">
                      {p.compras === 0 ? "sin compras" : `${p.compras} compras · última ${fechaHora(p.ultimaCompra!)}`}
                    </span>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </section>
      {panel && (
        <aside className="ml-4 w-96 shrink-0 tarjeta">
          <FormularioProveedor
            key={panel.tipo === "editar" ? panel.proveedor.id : "nuevo"}
            proveedor={panel.tipo === "editar" ? panel.proveedor : null}
            onGuardado={alGuardar}
            onCerrar={() => setPanel(null)}
          />
        </aside>
      )}
    </div>
  );
}

function FormularioProveedor({
  proveedor,
  onGuardado,
  onCerrar,
}: {
  proveedor: Proveedor | null;
  onGuardado: (p: Proveedor) => void;
  onCerrar: () => void;
}) {
  const [datos, setDatos] = useState<DatosProveedor>(
    proveedor
      ? {
          nombre: proveedor.nombre,
          rut: proveedor.rut ?? "",
          telefono: proveedor.telefono ?? "",
          contacto: proveedor.contacto ?? "",
          nota: proveedor.nota ?? "",
        }
      : VACIO,
  );
  const [error, setError] = useState<string | null>(null);
  const [guardando, setGuardando] = useState(false);
  const cambiar = (campo: keyof DatosProveedor) => (e: { target: { value: string } }) =>
    setDatos((d) => ({ ...d, [campo]: e.target.value }));

  const ejecutar = async (accion: () => Promise<Proveedor>) => {
    setGuardando(true);
    setError(null);
    try {
      onGuardado(await accion());
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
        void ejecutar(() => (proveedor ? api.editarProveedor(proveedor.id, datos) : api.crearProveedor(datos)));
      }}
      onKeyDown={(e) => e.key === "Escape" && onCerrar()}
      className="flex h-full flex-col gap-3 p-5"
    >
      <h2 className="font-semibold">{proveedor ? "Editar proveedor" : "Nuevo proveedor"}</h2>
      <Campo etiqueta="Nombre (obligatorio)">
        <input autoFocus value={datos.nombre} onChange={cambiar("nombre")} maxLength={80} className={claseCampo} />
      </Campo>
      <Campo etiqueta="RUT (opcional)">
        <input value={datos.rut ?? ""} onChange={cambiar("rut")} placeholder="76.123.456-0" maxLength={16} className={claseCampo} />
      </Campo>
      <Campo etiqueta="Teléfono (opcional)">
        <input value={datos.telefono ?? ""} onChange={cambiar("telefono")} maxLength={30} className={claseCampo} />
      </Campo>
      <Campo etiqueta="Contacto (opcional)">
        <input value={datos.contacto ?? ""} onChange={cambiar("contacto")} maxLength={80} className={claseCampo} />
      </Campo>
      <Campo etiqueta="Nota (opcional)">
        <input value={datos.nota ?? ""} onChange={cambiar("nota")} maxLength={300} className={claseCampo} />
      </Campo>
      {error && <p className="rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}
      <div className="mt-auto flex flex-wrap gap-2">
        <button
          type="submit"
          disabled={guardando}
          className="h-11 flex-1 rounded-lg bg-acento font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
        >
          Guardar
        </button>
        <button type="button" onClick={onCerrar} className="h-11 rounded-lg px-4 text-tenue hover:bg-fondo">
          Cerrar
        </button>
        {proveedor && (
          <button
            type="button"
            onClick={() => void ejecutar(() => api.cambiarEstadoProveedor(proveedor.id, !proveedor.activo))}
            className="h-10 w-full rounded-lg text-tenue hover:bg-fondo hover:text-texto"
          >
            {proveedor.activo ? "Desactivar (se conservan sus compras)" : "Volver a activar"}
          </button>
        )}
      </div>
    </form>
  );
}

function Campo({ etiqueta, children }: { etiqueta: string; children: ReactNode }) {
  return (
    <label className="block">
      <span className="text-xs text-tenue">{etiqueta}</span>
      <div className="mt-1">{children}</div>
    </label>
  );
}
