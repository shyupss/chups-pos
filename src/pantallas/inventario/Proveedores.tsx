import Boton from "../../componentes/Boton";
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api, textoError, type DatosProveedor, type Proveedor } from "../../api";
import Tabla, { type ColumnaTabla } from "../../componentes/Tabla";
import { textoConteo } from "../../componentes/tabla-nucleo";
import { fechaHora } from "../../formato";

const POR_PAGINA = 100;
const VACIO: DatosProveedor = { nombre: "", rut: "", telefono: "", contacto: "", nota: "" };
const claseCampo = "h-11 w-full rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento";

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

  const cargarMas = useCallback(() => {
    if (hayMas && !cargando.current && filas.length > 0) void cargar(filas.length);
  }, [hayMas, filas.length, cargar]);

  // Nombre · Compras · Última compra · Contacto · Teléfono.
  const columnas = useMemo<ColumnaTabla<Proveedor>[]>(
    () => [
      { id: "nombre", titulo: "Nombre", ancho: "minmax(10rem,24rem)", orden: (p) => p.nombre, celda: (p) => <span title={p.nombre}>{p.nombre}</span> },
      { id: "compras", titulo: "Compras", ancho: "6rem", alinear: "derecha", orden: (p) => p.compras, celda: (p) => p.compras },
      {
        id: "ultima",
        titulo: "Última compra",
        ancho: "10rem",
        orden: (p) => p.ultimaCompra,
        clase: () => "text-tenue",
        celda: (p) => (p.ultimaCompra ? fechaHora(p.ultimaCompra) : "sin compras"),
      },
      { id: "contacto", titulo: "Contacto", ancho: "minmax(8rem,14rem)", ocultarBajo: 740, orden: (p) => p.contacto, clase: () => "text-tenue", celda: (p) => p.contacto ?? "—" },
      { id: "telefono", titulo: "Teléfono", ancho: "9.5rem", ocultarBajo: 900, orden: (p) => p.telefono, clase: () => "text-tenue", celda: (p) => p.telefono ?? "—" },
    ],
    [],
  );

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
            className="h-11 min-w-64 flex-1 rounded-lg border border-borde-control bg-superficie px-4 focus:border-acento"
          />
          <label className="flex items-center gap-2 text-tenue">
            <input type="checkbox" checked={inactivos} onChange={(e) => setInactivos(e.target.checked)} />
            Ver desactivados
          </label>
          <Boton variante="primario" onClick={() => setPanel({ tipo: "nuevo" })}>
            Nuevo proveedor
          </Boton>
        </div>
        {error && <p className="mt-3 rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}
        <Tabla
          etiqueta="Proveedores"
          className="mt-4 flex-1"
          columnas={columnas}
          filas={filas}
          clave={(p) => p.id}
          onFila={(p) => setPanel({ tipo: "editar", proveedor: p })}
          seleccionada={(p) => panel?.tipo === "editar" && panel.proveedor.id === p.id}
          claseFila={(p) => (p.activo ? "" : "text-tenue line-through")}
          reiniciar={`${busqueda}|${inactivos}`}
          onCercaDelFinal={cargarMas}
          pie={textoConteo(filas.length, "proveedor", "proveedores", hayMas)}
          vacio={
            busqueda
              ? { texto: "No hay proveedores con esa búsqueda." }
              : { texto: "Todavía no hay proveedores.", accion: { etiqueta: "Nuevo proveedor", onClick: () => setPanel({ tipo: "nuevo" }) } }
          }
        />
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
      {error && <p className="rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}
      <div className="mt-auto flex flex-wrap gap-2">
        <Boton type="submit" variante="primario" disabled={guardando} className="flex-1">
          Guardar proveedor
        </Boton>
        <Boton variante="terciario" onClick={onCerrar}>
          Cerrar
        </Boton>
        {proveedor && (
          <Boton
            variante="terciario"
            bloque
            onClick={() => void ejecutar(() => api.cambiarEstadoProveedor(proveedor.id, !proveedor.activo))}
          >
            {proveedor.activo ? "Desactivar (se conservan sus compras)" : "Volver a activar"}
          </Boton>
        )}
      </div>
    </form>
  );
}

function Campo({ etiqueta, children }: { etiqueta: string; children: ReactNode }) {
  return (
    <label className="block">
      <span className="text-sm text-tenue">{etiqueta}</span>
      <div className="mt-1">{children}</div>
    </label>
  );
}
