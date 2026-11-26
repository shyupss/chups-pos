import Boton from "../../componentes/Boton";
import { Plus } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, textoError, type Compra, type DetalleCompra, type Proveedor } from "../../api";
import Dialogo from "../../componentes/Dialogo";
import Insignia from "../../componentes/Insignia";
import { esCancelado, useConPin } from "../../componentes/PedirPin";
import Tabla, { type ColumnaTabla } from "../../componentes/Tabla";
import { textoConteo } from "../../componentes/tabla-nucleo";
import { avisarCambioStock } from "../../eventos";
import { cantidad, fechaHora, pesos } from "../../formato";

const POR_PAGINA = 100;

interface Props {
  activa: boolean;
  /** Compra que se muestra en el detalle (desde un movimiento o recién registrada). */
  elegida: number | null;
  onElegir: (id: number | null) => void;
  onNueva: () => void;
  /** Cambia cuando se registra una compra: la lista se vuelve a cargar. */
  version: number;
}

/** Inventario > Compras: historial con filtros por proveedor y fechas, detalle y anulación. */
export default function Compras({ activa, elegida, onElegir, onNueva, version }: Props) {
  const [proveedores, setProveedores] = useState<Proveedor[]>([]);
  const [proveedorId, setProveedorId] = useState("");
  const [desde, setDesde] = useState("");
  const [hasta, setHasta] = useState("");
  const [filas, setFilas] = useState<Compra[]>([]);
  const [hayMas, setHayMas] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const cargando = useRef(false);
  const consultaActual = useRef(0);

  useEffect(() => {
    api.listarProveedores("", true, 200, 0).then(setProveedores).catch(() => {});
  }, []);

  const cargar = useCallback(
    async (desdeFila: number) => {
      const consulta = desdeFila === 0 ? ++consultaActual.current : consultaActual.current;
      cargando.current = true;
      try {
        const pagina = await api.listarCompras(
          { proveedorId: proveedorId ? Number(proveedorId) : null, desde: desde || null, hasta: hasta || null },
          POR_PAGINA,
          desdeFila,
        );
        if (consulta !== consultaActual.current) return;
        setFilas((f) => (desdeFila === 0 ? pagina : [...f, ...pagina]));
        setHayMas(pagina.length === POR_PAGINA);
        setError(null);
      } catch (e) {
        setError(textoError(e));
      } finally {
        cargando.current = false;
      }
    },
    [proveedorId, desde, hasta],
  );

  useEffect(() => {
    if (activa) void cargar(0);
  }, [activa, cargar, version]);

  const cargarMas = useCallback(() => {
    if (hayMas && !cargando.current && filas.length > 0) void cargar(filas.length);
  }, [hayMas, filas.length, cargar]);

  // Fecha · Proveedor · Total · Productos · Documento (y el N° al final, que es lo secundario).
  const columnas = useMemo<ColumnaTabla<Compra>[]>(
    () => [
      { id: "fecha", titulo: "Fecha", ancho: "9.5rem", orden: (c) => c.fecha, celda: (c) => fechaHora(c.fecha) },
      {
        id: "proveedor",
        titulo: "Proveedor",
        ancho: "minmax(10rem,20rem)",
        sinPuntos: true,
        orden: (c) => c.proveedor,
        celda: (c) => (
          <span className="flex min-w-0 items-center gap-2">
            <span className="truncate">{c.proveedor ?? <span className="text-tenue">Sin proveedor</span>}</span>
            {c.anuladaEn && <Insignia tono="peligro">Anulada</Insignia>}
            {c.pagadaConCaja && !c.anuladaEn && <Insignia tono="neutro">Con la caja</Insignia>}
          </span>
        ),
      },
      {
        id: "total",
        titulo: "Total",
        ancho: "7.5rem",
        alinear: "derecha",
        orden: (c) => c.total,
        clase: (c) => (c.anuladaEn ? "line-through text-tenue" : "font-medium"),
        celda: (c) => pesos(c.total),
      },
      { id: "productos", titulo: "Productos", ancho: "6rem", alinear: "derecha", ocultarBajo: 640, orden: (c) => c.lineas, celda: (c) => c.lineas },
      { id: "documento", titulo: "Documento", ancho: "8rem", ocultarBajo: 780, orden: (c) => c.documento, clase: () => "text-tenue", celda: (c) => c.documento ?? "—" },
      { id: "numero", titulo: "N°", ancho: "4rem", alinear: "derecha", ocultarBajo: 900, orden: (c) => c.id, clase: () => "text-tenue", celda: (c) => c.id },
    ],
    [],
  );

  return (
    <div className="flex min-h-0 flex-1">
      <section className="@container flex min-w-0 flex-1 flex-col">
        <div className="flex flex-wrap items-center gap-3">
          <select
            value={proveedorId}
            onChange={(e) => setProveedorId(e.target.value)}
            aria-label="Proveedor"
            className="h-11 max-w-64 rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento"
          >
            <option value="">Todos los proveedores</option>
            {proveedores.map((p) => (
              <option key={p.id} value={p.id}>
                {p.nombre}
                {p.activo ? "" : " (desactivado)"}
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
          <Boton variante="primario" icono={Plus} atajo="Ctrl N" onClick={onNueva} aria-keyshortcuts="Control+N" className="ml-auto">
            Nueva compra
          </Boton>
        </div>
        {error && <p className="mt-3 rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}

        <Tabla
          etiqueta="Compras"
          className="mt-4 flex-1"
          columnas={columnas}
          filas={filas}
          clave={(c) => c.id}
          onFila={(c) => onElegir(c.id)}
          seleccionada={(c) => elegida === c.id}
          claseFila={(c) => (c.anuladaEn ? "text-tenue" : "")}
          reiniciar={`${proveedorId}|${desde}|${hasta}|${version}`}
          onCercaDelFinal={cargarMas}
          pie={textoConteo(filas.length, "compra", "compras", hayMas)}
          vacio={
            proveedorId || desde || hasta
              ? { texto: "No hay compras con estos filtros." }
              : { texto: "Todavía no hay compras.", accion: { etiqueta: "Nueva compra", onClick: onNueva } }
          }
        />
      </section>
      {elegida !== null && (
        <aside className="ml-4 w-96 shrink-0 overflow-hidden tarjeta">
          <DetalleCompraPanel
            key={elegida}
            id={elegida}
            onCerrar={() => onElegir(null)}
            onAnulada={() => {
              avisarCambioStock();
              void cargar(0);
            }}
          />
        </aside>
      )}
    </div>
  );
}

function DetalleCompraPanel({ id, onCerrar, onAnulada }: { id: number; onCerrar: () => void; onAnulada: () => void }) {
  const conPin = useConPin();
  const [d, setD] = useState<DetalleCompra | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [anulando, setAnulando] = useState(false);
  const [motivo, setMotivo] = useState("");

  useEffect(() => {
    api
      .detalleCompra(id)
      .then(setD)
      .catch((e) => setError(textoError(e)));
  }, [id]);

  const anular = async () => {
    if (!motivo.trim()) return;
    setError(null);
    try {
      setD(await conPin((pin) => api.anularCompra(id, motivo, pin), { accion: `Anular la compra N° ${id}` }));
      setAnulando(false);
      onAnulada();
    } catch (e) {
      if (!esCancelado(e)) setError(textoError(e));
    }
  };

  if (!d) return <p className="p-5 text-tenue">{error ?? "Cargando…"}</p>;
  const c = d.compra;
  return (
    <div className="flex h-full flex-col">
      <div className="flex h-14 shrink-0 items-center gap-3 border-b border-linea px-5">
        <h2 className="font-semibold">Compra N° {c.id}</h2>
        {c.anuladaEn && <span className="rounded-full bg-peligro-fondo px-2.5 py-0.5 text-xs font-medium text-peligro">Anulada</span>}
        <Boton tamano="icono" onClick={onCerrar} aria-label="Cerrar detalle" className="ml-auto">
          ×
        </Boton>
      </div>
      <div className="min-h-0 flex-1 space-y-4 overflow-y-auto p-5 tabular-nums">
        <dl className="space-y-1">
          <Par etiqueta="Fecha" valor={fechaHora(c.fecha)} />
          <Par etiqueta="Proveedor" valor={c.proveedor ?? "Sin proveedor"} />
          {c.documento && <Par etiqueta="Documento" valor={c.documento} />}
          <Par etiqueta="Pago" valor={c.pagadaConCaja ? "Efectivo de la caja" : "Fuera de la caja"} />
        </dl>
        <ul className="divide-y divide-linea">
          {d.items.map((i) => (
            <li key={i.productoId} className="py-2">
              <div className="flex justify-between gap-3">
                <span className="min-w-0">{i.nombre}</span>
                <span className="font-medium">{pesos(i.subtotal)}</span>
              </div>
              <div className="text-xs text-tenue">
                {cantidad(i.cantidadMil, i.unidad)} x {pesos(i.costoUnitario)}
                {i.unidad === "kg" ? "/kg" : ""}
              </div>
            </li>
          ))}
        </ul>
        <div className="flex items-baseline justify-between text-lg font-semibold">
          <span>Total</span>
          <span>{pesos(c.total)}</span>
        </div>
        {c.nota && <p className="text-tenue">Nota: {c.nota}</p>}
        {c.anuladaEn && (
          <p className="rounded-lg bg-peligro-fondo p-3 text-peligro">
            Anulada el {fechaHora(c.anuladaEn)}: {c.motivoAnulacion}
          </p>
        )}
        {error && <p className="rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}
      </div>
      {!c.anuladaEn && (
        <div className="shrink-0 border-t border-linea p-4">
          <Boton variante="peligro" bloque onClick={() => setAnulando(true)}>
            Anular compra
          </Boton>
        </div>
      )}
      {anulando && (
        <Dialogo titulo={`Anular compra N° ${c.id}`} onCerrar={() => setAnulando(false)}>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void anular();
            }}
            className="mt-3 space-y-3"
          >
            <p className="text-tenue">
              Se descuenta del stock lo que entró con esta compra
              {c.pagadaConCaja ? ` y vuelven ${pesos(c.total)} a la caja abierta` : ""}.
            </p>
            {d.quedarianNegativos.length > 0 && (
              <p className="rounded-lg bg-aviso-fondo px-3 py-2 text-aviso">
                Quedarían con stock negativo: {d.quedarianNegativos.join(", ")}.
              </p>
            )}
            <input
              autoFocus
              value={motivo}
              onChange={(e) => setMotivo(e.target.value)}
              maxLength={200}
              placeholder="Motivo (obligatorio)"
              aria-label="Motivo de la anulación"
              className="h-11 w-full rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento"
            />
            {error && <p className="rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}
            <div className="flex gap-2">
              <Boton variante="terciario" atajo="Esc" onClick={() => setAnulando(false)} className="flex-1">
                Volver
              </Boton>
              <Boton type="submit" variante="peligro" disabled={!motivo.trim()} className="flex-[2]">
                Anular compra
              </Boton>
            </div>
          </form>
        </Dialogo>
      )}
    </div>
  );
}

function Par({ etiqueta, valor }: { etiqueta: string; valor: string }) {
  return (
    <div className="flex justify-between gap-3">
      <dt className="text-tenue">{etiqueta}</dt>
      <dd className="text-right">{valor}</dd>
    </div>
  );
}
