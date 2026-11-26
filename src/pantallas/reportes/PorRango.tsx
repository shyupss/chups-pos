// Las pestañas de reportes por rango: Ventas, Productos, Ganancia, Turnos y Fiados. Cada una
// pide su reporte en una llamada y lo dibuja; la exportación la hace Rust (Excel y PDF).

import { ChevronRight } from "lucide-react";
import { useMemo, useState } from "react";
import Boton from "../../componentes/Boton";
import { api, type Categoria, type ProductoVendido } from "../../api";
import { Barras, DIAS, Lineas, MapaCalor } from "../../componentes/Graficos";
import Opciones from "../../componentes/Opciones";
import { cantidad, fechaHora, folio, NOMBRE_MEDIO, pesos, pesosConSigno } from "../../formato";
import { Panel, Tarjeta } from "./Hoy";
import { pesosCortos, porcentaje } from "./formato";
import { Exportar, useReporte, type Rango } from "./Rango";
import { aFecha, completarDias, type VentaDia } from "./reportes-nucleo";
import TablaVirtual from "./TablaVirtual";

const DIA_LARGO = new Intl.DateTimeFormat("es-CL", { weekday: "long", day: "numeric", month: "long" });
const DIA_CORTO = new Intl.DateTimeFormat("es-CL", { day: "numeric", month: "short" });
const dia = aFecha;
const boletas = (n: number) => (n === 1 ? "1 boleta" : `${n} boletas`);
const mayuscula = (t: string) => t.charAt(0).toUpperCase() + t.slice(1);

/** Lo que mide cada tarjeta, en una frase (el ícono (i) las muestra). */
export const AYUDA = {
  vendido: "Todo lo que vendiste en el período: la suma de las ventas, sin contar las anuladas y descontando las devoluciones.",
  ventas: "Cuántas ventas (boletas) hiciste en el período. No cuenta las anuladas.",
  ticket:
    "Lo que gasta en promedio cada cliente por compra: total vendido dividido por la cantidad de ventas del período. No cuenta ventas anuladas y descuenta las devoluciones. También se le dice «ticket promedio».",
  ganancia: "Lo vendido menos lo que te costaron esos productos. Solo cuenta productos con costo registrado.",
  margen: "De cada $100 que vendiste (con costo registrado), cuántos pesos son ganancia.",
} as const;

function Estado({ error }: { error: string | null }) {
  return error ? <p className="rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p> : <p className="text-tenue">Cargando…</p>;
}

// --- Ventas ---------------------------------------------------------------------------------

export function ReporteVentas({ activa, rango }: { activa: boolean; rango: Rango }) {
  const { datos: r, error } = useReporte(() => api.reporteVentas(rango.desde, rango.hasta), [rango.desde, rango.hasta]);
  const [medida, setMedida] = useState<"monto" | "boletas">("monto");
  const dias = useMemo(() => (r ? completarDias(r.porDia, rango.desde, rango.hasta) : []), [r, rango.desde, rango.hasta]);
  if (!r) return <Estado error={error} />;
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="grid flex-1 grid-cols-2 gap-3 lg:grid-cols-3">
          <Tarjeta titulo="Vendido" valor={pesos(r.total)} pie="Sin las anuladas" ayuda={AYUDA.vendido} />
          <Tarjeta titulo="Ventas" valor={r.boletas.toLocaleString("es-CL")} pie="Boletas del período" ayuda={AYUDA.ventas} />
          <Tarjeta titulo="Gasto promedio por venta" valor={pesos(r.ticketPromedio)} ayuda={AYUDA.ticket} />
        </div>
        <Exportar activa={activa} tipo="ventas" rango={rango} nombre="Ventas" />
      </div>
      <Panel titulo="¿Cuánto vendí cada día?">
        <GraficoDias dias={dias} />
      </Panel>
      <div className="grid gap-3 lg:grid-cols-3">
        <Panel titulo="¿Con qué me pagan?">
          <Barras
            filas={r.porMedio.map((m) => ({ etiqueta: NOMBRE_MEDIO[m.medio], valor: m.monto, detalle: `${pesos(m.monto)} · ${boletas(m.boletas)}` }))}
            formato={pesos}
          />
        </Panel>
        <Panel titulo="¿Qué categorías venden más?">
          <Barras filas={r.porCategoria.map((c) => ({ etiqueta: c.nombre, valor: c.monto }))} formato={pesos} />
        </Panel>
        <Panel titulo="¿Qué días de la semana se vende más?">
          <Barras
            filas={r.porDiaSemana.map((c, i) => ({ etiqueta: DIAS[i], valor: c.monto, detalle: `${pesos(c.monto)} · ${boletas(c.boletas)}` }))}
            formato={pesos}
          />
        </Panel>
      </div>
      <details className="tarjeta group">
        <summary className="flex h-12 cursor-pointer list-none items-center gap-2 px-4 font-semibold marker:hidden [&::-webkit-details-marker]:hidden">
          <ChevronRight size={18} aria-hidden className="text-tenue group-open:rotate-90" />
          Ver a qué horas se vende más
        </summary>
        <div className="border-t border-linea p-4">
          <div className="mb-3">
            <Opciones
              nombre="medida-mapa"
              etiqueta="Qué mostrar en el mapa"
              opciones={[
                { id: "monto", nombre: "Ventas" },
                { id: "boletas", nombre: "Boletas" },
              ]}
              valor={medida}
              onCambiar={setMedida}
            />
          </div>
          <MapaCalor
            celdas={r.mapa}
            valor={(c) => (medida === "monto" ? c.monto : c.boletas)}
            detalle={(c) => `${pesos(c.monto)} · ${boletas(c.boletas)}`}
          />
        </div>
      </details>
    </div>
  );
}

/** Las ventas de cada día como una línea; los días sin ventas valen 0. Se usa en Resumen y en Ventas. */
export function GraficoDias({
  dias,
  anteriores,
  nombreActual = "Este período",
  nombreAnterior,
}: {
  dias: VentaDia[];
  anteriores?: VentaDia[];
  nombreActual?: string;
  nombreAnterior?: string;
}) {
  const largo = Math.max(dias.length, anteriores?.length ?? 0);
  const etiquetas = Array.from({ length: largo }, (_, i) => (largo > 31 ? DIA_CORTO.format(dia((dias[i] ?? anteriores?.[i])?.dia ?? "2000-01-01")) : String(i + 1)));
  const titulos = Array.from({ length: largo }, (_, i) => {
    const d = dias[i]?.dia;
    return d ? mayuscula(DIA_LARGO.format(dia(d))) : `Día ${i + 1}`;
  });
  return (
    <Lineas
      descripcion="Ventas de cada día del período"
      etiquetas={etiquetas}
      titulos={titulos}
      formatoEje={pesosCortos}
      formatoValor={pesos}
      series={[
        { nombre: nombreActual, valores: dias.map((d) => d.monto) },
        ...(anteriores ? [{ nombre: nombreAnterior ?? "Período anterior", valores: anteriores.map((d) => d.monto), punteada: true }] : []),
      ]}
    />
  );
}

// --- Productos ------------------------------------------------------------------------------

export function ReporteProductos({
  activa,
  rango,
  categorias,
  onVerProducto,
}: {
  activa: boolean;
  rango: Rango;
  categorias: Categoria[];
  onVerProducto: (p: { id: number; nombre: string }) => void;
}) {
  const [categoria, setCategoria] = useState<number | null>(null);
  const [orden, setOrden] = useState<"monto" | "cantidad">("monto");
  const { datos: r, error } = useReporte(
    () => api.reporteProductos(rango.desde, rango.hasta, categoria),
    [rango.desde, rango.hasta, categoria],
  );
  const filas = useMemo(() => {
    if (!r) return [];
    const v = [...r.productos];
    // Por cantidad: los de kg y los de unidad no se comparan entre sí, se ordena cada grupo.
    if (orden === "cantidad") v.sort((a, b) => (a.unidad === b.unidad ? b.cantidadMil - a.cantidadMil : a.unidad === "unidad" ? -1 : 1));
    return v;
  }, [r, orden]);
  const total = r?.productos.reduce((s, p) => s + p.monto, 0) ?? 0;
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap items-center gap-3">
          <select
            aria-label="Categoría"
            value={categoria === null ? "todas" : String(categoria)}
            onChange={(e) => setCategoria(e.target.value === "todas" ? null : Number(e.target.value))}
            className="h-9 tarjeta px-3"
          >
            <option value="todas">Todas las categorías</option>
            {categorias.map((c) => (
              <option key={c.id} value={c.id}>
                {c.nombre}
              </option>
            ))}
            <option value="0">Sin categoría</option>
          </select>
          <Opciones
            nombre="orden-productos"
            etiqueta="Ordenar por"
            opciones={[
              { id: "monto", nombre: "Por monto" },
              { id: "cantidad", nombre: "Por cantidad" },
            ]}
            valor={orden}
            onCambiar={setOrden}
          />
        </div>
        <Exportar activa={activa} tipo="productos" rango={rango} categoria={categoria} nombre="Productos" />
      </div>
      {!r ? (
        <Estado error={error} />
      ) : (
        <>
          <Panel titulo={`¿Qué se vende más? (${r.productos.length === 1 ? "1 producto" : `${r.productos.length} productos`})`}>
            <TablaVirtual
              filas={filas}
              clave={(p) => p.productoId}
              vacia="No hubo ventas en este período."
              columnas={[
                { titulo: "#", ancho: "2.5rem", celda: (_, i) => <span className="text-tenue">{i + 1}</span> },
                {
                  titulo: "Producto",
                  ancho: "minmax(0,1fr)",
                  orden: (p) => p.nombre,
                  celda: (p) => (
                    <Boton variante="enlace" onClick={() => onVerProducto({ id: p.productoId, nombre: p.nombre })} className="whitespace-nowrap" title="Ver movimientos">
                      {p.nombre}
                    </Boton>
                  ),
                },
                { titulo: "Categoría", ancho: "9rem", orden: (p) => p.categoria, celda: (p) => <span className="text-tenue">{p.categoria ?? "—"}</span> },
                { titulo: "Cantidad", ancho: "7rem", derecha: true, orden: (p) => p.cantidadMil, celda: (p) => cantidad(p.cantidadMil, p.unidad) },
                { titulo: "Ventas", ancho: "7rem", derecha: true, orden: (p) => p.monto, celda: (p) => <span className="font-medium">{pesos(p.monto)}</span> },
                {
                  titulo: "% del total",
                  ancho: "5.5rem",
                  derecha: true,
                  orden: (p) => p.monto,
                  celda: (p) => <span className="text-tenue">{total ? porcentaje(Math.round((p.monto * 10000) / total)) : "—"}</span>,
                },
              ]}
            />
          </Panel>
          <Panel titulo={`¿Qué no se vendió? (${r.sinVentas.length})`}>
            <TablaVirtual
              filas={r.sinVentas}
              clave={(p) => p.productoId}
              vacia="Todos tus productos activos se vendieron."
              alto={300}
              columnas={[
                {
                  titulo: "Producto",
                  ancho: "minmax(0,1fr)",
                  orden: (p) => p.nombre,
                  celda: (p) => (
                    <Boton variante="enlace" onClick={() => onVerProducto({ id: p.productoId, nombre: p.nombre })} className="whitespace-nowrap" title="Ver movimientos">
                      {p.nombre}
                    </Boton>
                  ),
                },
                { titulo: "Categoría", ancho: "9rem", orden: (p) => p.categoria, celda: (p) => <span className="text-tenue">{p.categoria ?? "—"}</span> },
                { titulo: "Stock", ancho: "7rem", derecha: true, orden: (p) => p.stockMil, celda: (p) => cantidad(p.stockMil, p.unidad) },
              ]}
            />
          </Panel>
        </>
      )}
    </div>
  );
}

// --- Ganancia -------------------------------------------------------------------------------

export function ReporteGanancia({ activa, rango }: { activa: boolean; rango: Rango }) {
  const { datos: r, error } = useReporte(() => api.reporteGanancia(rango.desde, rango.hasta), [rango.desde, rango.hasta]);
  if (!r) return <Estado error={error} />;
  const columnaGanancia = (p: ProductoVendido) => (
    <span className={`font-medium ${(p.ganancia ?? 0) < 0 ? "text-peligro" : ""}`}>{pesos(p.ganancia ?? 0)}</span>
  );
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="grid flex-1 grid-cols-2 gap-3 lg:grid-cols-4">
          <Tarjeta titulo="Ganancia" valor={pesos(r.ganancia)} ayuda={AYUDA.ganancia} />
          <Tarjeta titulo="Margen" valor={r.margenPb === null ? "—" : porcentaje(r.margenPb)} ayuda={AYUDA.margen} />
          <Tarjeta titulo="Vendido con costo" valor={pesos(r.ventas)} pie="Lo que cuenta para la ganancia" />
          <Tarjeta titulo="Lo que costó" valor={pesos(r.costo)} pie="El costo de esos productos" />
        </div>
        <Exportar activa={activa} tipo="ganancia" rango={rango} nombre="Ganancia" />
      </div>
      {(r.ventasEstimadas > 0 || r.ventasSinCosto > 0) && (
        <div className="space-y-2">
          {r.ventasEstimadas > 0 && (
            <p className="rounded-lg bg-aviso-fondo px-3 py-2 text-aviso">
              {pesos(r.ventasEstimadas)} de estas ventas usan costos estimados ({pesos(r.costoEstimado)} de costo): son de antes de que cada
              venta guardara su costo, así que se calcularon con el costo actual del producto.
            </p>
          )}
          {r.ventasSinCosto > 0 && (
            <p className="rounded-lg bg-suave px-3 py-2 text-tenue">
              {pesos(r.ventasSinCosto)} vendidos de {r.sinCosto.length === 1 ? "1 producto" : `${r.sinCosto.length} productos`} sin costo no
              cuentan en la ganancia: sin costo no se puede saber cuánto se ganó.
            </p>
          )}
        </div>
      )}
      <div className="grid gap-3 lg:grid-cols-3">
        <Panel titulo="¿Qué productos dejan más ganancia?" className="lg:col-span-2">
          <TablaVirtual
            filas={r.productos}
            clave={(p) => p.productoId}
            vacia="No hay ventas con costo en este período."
            columnas={[
              {
                titulo: "Producto",
                ancho: "minmax(0,1fr)",
                orden: (p) => p.nombre,
                celda: (p) => (
                  <span title={p.costoEstimado ? "Parte del costo es estimada" : undefined}>
                    {p.nombre}
                    {p.costoEstimado && <span className="ml-1 text-aviso">*</span>}
                  </span>
                ),
              },
              { titulo: "Ventas", ancho: "6.5rem", derecha: true, orden: (p) => p.montoConCosto, celda: (p) => pesos(p.montoConCosto) },
              { titulo: "Costo", ancho: "6.5rem", derecha: true, orden: (p) => p.costo, celda: (p) => <span className="text-tenue">{pesos(p.costo ?? 0)}</span> },
              { titulo: "Ganancia", ancho: "6.5rem", derecha: true, celda: columnaGanancia },
              { titulo: "Margen", ancho: "4.5rem", derecha: true, celda: (p) => (p.margenPb === null ? "—" : porcentaje(p.margenPb)) },
            ]}
          />
          {r.productos.some((p) => p.costoEstimado) && <p className="mt-2 text-xs text-tenue">* Parte del costo es estimada.</p>}
        </Panel>
        <div className="space-y-3">
          <Panel titulo="¿Qué categorías dejan más ganancia?">
            <Barras
              filas={r.porCategoria.map((c) => ({
                etiqueta: `${c.nombre}${c.margenPb === null ? "" : ` · ${porcentaje(c.margenPb)}`}`,
                valor: c.ganancia,
                detalle: `Ventas ${pesos(c.ventas)} · costo ${pesos(c.costo)}`,
              }))}
              formato={pesos}
            />
          </Panel>
          {r.sinCosto.length > 0 && (
            <Panel titulo="Productos sin costo (no cuentan en la ganancia)">
              <ul className="max-h-64 space-y-1 overflow-auto">
                {r.sinCosto.map((p) => (
                  <li key={p.productoId} className="flex justify-between gap-3 text-sm">
                    <span className="min-w-0 truncate">{p.nombre}</span>
                    <span className="text-tenue tabular-nums">{pesos(p.montoSinCosto)}</span>
                  </li>
                ))}
              </ul>
            </Panel>
          )}
        </div>
      </div>
    </div>
  );
}

// --- Turnos ---------------------------------------------------------------------------------

export function ReporteTurnos({ activa, rango }: { activa: boolean; rango: Rango }) {
  const { datos: r, error } = useReporte(() => api.reporteTurnos(rango.desde, rango.hasta), [rango.desde, rango.hasta]);
  if (!r) return <Estado error={error} />;
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="grid flex-1 grid-cols-2 gap-3 lg:grid-cols-4">
          <Tarjeta titulo="Turnos cerrados" valor={String(r.turnos.length)} ayuda="Cuántas veces cerraste la caja en el período." />
          <Tarjeta titulo="Cuadrados" valor={String(r.cuadrados)} ayuda="Cierres en que la plata contada fue justo la que debía haber." />
          <Tarjeta titulo="Sobrantes" valor={pesos(r.sobrantes)} />
          <Tarjeta titulo="Faltantes" valor={<span className={r.faltantes > 0 ? "text-peligro" : ""}>{pesos(r.faltantes)}</span>} />
        </div>
        <Exportar activa={activa} tipo="turnos" rango={rango} nombre="Turnos" />
      </div>
      <Panel titulo="¿Cómo cerraron las cajas?">
        <TablaVirtual
          filas={r.turnos}
          clave={(t) => t.id}
          vacia="No se cerró ningún turno en este período."
          columnas={[
            { titulo: "Turno", ancho: "6rem", orden: (t) => t.id, celda: (t) => folio(t.id) },
            { titulo: "Apertura", ancho: "minmax(0,1fr)", orden: (t) => t.abiertoEn, celda: (t) => fechaHora(t.abiertoEn) },
            { titulo: "Cierre", ancho: "minmax(0,1fr)", orden: (t) => t.cerradoEn, celda: (t) => fechaHora(t.cerradoEn) },
            { titulo: "Esperado", ancho: "7rem", derecha: true, orden: (t) => t.esperado, celda: (t) => pesos(t.esperado) },
            { titulo: "Contado", ancho: "7rem", derecha: true, orden: (t) => t.contado, celda: (t) => pesos(t.contado) },
            {
              titulo: "Diferencia",
              ancho: "7rem",
              derecha: true,
              orden: (t) => t.diferencia,
              celda: (t) => (
                <span className={`font-medium ${t.diferencia < 0 ? "text-peligro" : t.diferencia > 0 ? "text-aviso" : "text-exito"}`}>
                  {t.diferencia === 0 ? "Cuadrada" : pesosConSigno(t.diferencia)}
                </span>
              ),
            },
          ]}
        />
      </Panel>
    </div>
  );
}

// --- Fiados ---------------------------------------------------------------------------------

export function ReporteFiados({ activa, rango, onVerCliente }: { activa: boolean; rango: Rango; onVerCliente: (id: number) => void }) {
  const { datos: r, error } = useReporte(() => api.reporteFiados(rango.desde, rango.hasta), [rango.desde, rango.hasta]);
  if (!r) return <Estado error={error} />;
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="grid flex-1 grid-cols-2 gap-3 lg:grid-cols-4">
          <Tarjeta titulo="Deuda al cierre" ayuda="Lo que te debían los clientes al último día del período." valor={pesos(r.deudaAlCierre)} pie={`${r.clientesConDeuda} clientes al ${DIA_CORTO.format(dia(r.hasta))}`} />
          <Tarjeta titulo="Fiado otorgado" valor={pesos(r.otorgado)} ayuda="Lo que vendiste fiado en el período." />
          <Tarjeta titulo="Pagos recibidos" valor={pesos(r.pagado)} ayuda="Lo que los clientes te pagaron de sus deudas en el período." />
          <Tarjeta titulo="Cómo cambió la deuda" valor={pesosConSigno(r.otorgado - r.pagado)} pie="Fiado menos pagado" ayuda="Si es positivo, te deben más que al empezar; si es negativo, te deben menos." />
        </div>
        <Exportar activa={activa} tipo="fiados" rango={rango} nombre="Fiados" />
      </div>
      <div className="grid gap-3 lg:grid-cols-2">
        <Panel titulo="¿Hace cuánto deben?">
          <Barras
            filas={r.antiguedad.map((t) => ({ etiqueta: `${t.tramo} días`, valor: t.monto, detalle: `${pesos(t.monto)} en ${t.ventas} ventas` }))}
            formato={pesos}
          />
        </Panel>
        <Panel titulo="¿Quiénes deben más?">
          {r.deudores.length === 0 ? (
            <p className="text-tenue">Nadie debía nada.</p>
          ) : (
            <ul className="space-y-2">
              {r.deudores.map((d) => (
                <li key={d.clienteId} className="flex items-baseline gap-3">
                  <span className="min-w-0 flex-1 truncate">
                    <Boton variante="enlace" onClick={() => onVerCliente(d.clienteId)} className="whitespace-nowrap">
                      {d.nombre}
                    </Boton>
                  </span>
                  <span className="font-medium tabular-nums">{pesos(d.deuda)}</span>
                </li>
              ))}
            </ul>
          )}
        </Panel>
      </div>
    </div>
  );
}
