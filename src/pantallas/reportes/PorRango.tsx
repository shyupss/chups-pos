// Las pestañas de reportes por rango: Ventas, Productos, Ganancia, Turnos y Fiados. Cada una
// pide su reporte en una llamada y lo dibuja; la exportación la hace Rust (Excel y PDF).

import { useMemo, useState } from "react";
import Boton from "../../componentes/Boton";
import { api, type Categoria, type ProductoVendido } from "../../api";
import { Barras, Columnas, DIAS, MapaCalor } from "../../componentes/Graficos";
import Opciones from "../../componentes/Opciones";
import { cantidad, fechaHora, folio, NOMBRE_MEDIO, pesos, pesosConSigno } from "../../formato";
import { Panel, Tarjeta } from "./Hoy";
import { pesosCortos, porcentaje } from "./formato";
import { Exportar, useReporte, type Rango } from "./Rango";
import TablaVirtual from "./TablaVirtual";

const DIA_CORTO = new Intl.DateTimeFormat("es-CL", { day: "numeric", month: "short" });
const dia = (texto: string) => new Date(`${texto}T12:00:00`);
const boletas = (n: number) => (n === 1 ? "1 boleta" : `${n} boletas`);

function Estado({ error }: { error: string | null }) {
  return error ? <p className="rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p> : <p className="text-tenue">Cargando…</p>;
}

// --- Ventas ---------------------------------------------------------------------------------

export function ReporteVentas({ activa, rango }: { activa: boolean; rango: Rango }) {
  const { datos: r, error } = useReporte(() => api.reporteVentas(rango.desde, rango.hasta), [rango.desde, rango.hasta]);
  const [medida, setMedida] = useState<"monto" | "boletas">("monto");
  if (!r) return <Estado error={error} />;
  const cada = Math.max(1, Math.ceil(r.porDia.length / 16));
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="grid flex-1 grid-cols-3 gap-3">
          <Tarjeta titulo="Total vendido" valor={pesos(r.total)} pie="Sin anuladas, menos devoluciones" />
          <Tarjeta titulo="Boletas" valor={r.boletas.toLocaleString("es-CL")} />
          <Tarjeta titulo="Ticket promedio" valor={pesos(r.ticketPromedio)} />
        </div>
        <Exportar activa={activa} tipo="ventas" rango={rango} />
      </div>
      <Panel titulo="Ventas por día">
        <Columnas
          descripcion="Ventas por día"
          columnas={r.porDia.map((d) => ({
            etiqueta: DIA_CORTO.format(dia(d.dia)),
            valor: d.monto,
            detalle: `${DIA_CORTO.format(dia(d.dia))} · ${pesos(d.monto)} · ${boletas(d.boletas)}`,
          }))}
          etiquetaCada={cada}
          formatoEje={pesosCortos}
        />
      </Panel>
      <div className="grid gap-3 lg:grid-cols-3">
        <Panel titulo="Por medio de pago">
          <Barras
            filas={r.porMedio.map((m) => ({ etiqueta: NOMBRE_MEDIO[m.medio], valor: m.monto, detalle: `${pesos(m.monto)} · ${boletas(m.boletas)}` }))}
            formato={pesos}
          />
        </Panel>
        <Panel titulo="Por categoría">
          <Barras filas={r.porCategoria.map((c) => ({ etiqueta: c.nombre, valor: c.monto }))} formato={pesos} />
        </Panel>
        <Panel titulo="Por día de la semana">
          <Barras
            filas={r.porDiaSemana.map((c, i) => ({ etiqueta: DIAS[i], valor: c.monto, detalle: `${pesos(c.monto)} · ${boletas(c.boletas)}` }))}
            formato={pesos}
          />
        </Panel>
      </div>
      <Panel titulo="Día de la semana y hora">
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
      </Panel>
    </div>
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
        <Exportar activa={activa} tipo="productos" rango={rango} categoria={categoria} />
      </div>
      {!r ? (
        <Estado error={error} />
      ) : (
        <>
          <Panel titulo={`Vendidos (${r.productos.length})`}>
            <TablaVirtual
              filas={filas}
              clave={(p) => p.productoId}
              vacia="No hubo ventas en este rango."
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
          <Panel titulo={`Sin ventas en el rango (${r.sinVentas.length})`}>
            <TablaVirtual
              filas={r.sinVentas}
              clave={(p) => p.productoId}
              vacia="Todos los productos activos tuvieron ventas."
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
        <div className="grid flex-1 grid-cols-2 gap-3 xl:grid-cols-4">
          <Tarjeta titulo="Ventas con costo" valor={pesos(r.ventas)} />
          <Tarjeta titulo="Costo" valor={pesos(r.costo)} />
          <Tarjeta titulo="Ganancia" valor={pesos(r.ganancia)} />
          <Tarjeta titulo="Margen" valor={r.margenPb === null ? "—" : porcentaje(r.margenPb)} />
        </div>
        <Exportar activa={activa} tipo="ganancia" rango={rango} />
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
        <Panel titulo="Por producto" className="lg:col-span-2">
          <TablaVirtual
            filas={r.productos}
            clave={(p) => p.productoId}
            vacia="No hay ventas con costo en este rango."
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
          <Panel titulo="Ganancia por categoría">
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
            <Panel titulo="Productos sin costo">
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
        <div className="grid flex-1 grid-cols-2 gap-3 xl:grid-cols-4">
          <Tarjeta titulo="Turnos cerrados" valor={String(r.turnos.length)} />
          <Tarjeta titulo="Cuadrados" valor={String(r.cuadrados)} />
          <Tarjeta titulo="Sobrantes" valor={pesos(r.sobrantes)} />
          <Tarjeta titulo="Faltantes" valor={<span className={r.faltantes > 0 ? "text-peligro" : ""}>{pesos(r.faltantes)}</span>} />
        </div>
        <Exportar activa={activa} tipo="turnos" rango={rango} />
      </div>
      <Panel titulo="Turnos">
        <TablaVirtual
          filas={r.turnos}
          clave={(t) => t.id}
          vacia="No se cerraron turnos en este rango."
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
        <div className="grid flex-1 grid-cols-2 gap-3 xl:grid-cols-4">
          <Tarjeta titulo="Deuda al cierre" valor={pesos(r.deudaAlCierre)} pie={`${r.clientesConDeuda} clientes al ${DIA_CORTO.format(dia(r.hasta))}`} />
          <Tarjeta titulo="Fiado en el rango" valor={pesos(r.otorgado)} />
          <Tarjeta titulo="Pagado en el rango" valor={pesos(r.pagado)} />
          <Tarjeta titulo="Diferencia" valor={pesosConSigno(r.otorgado - r.pagado)} pie="Fiado menos pagado" />
        </div>
        <Exportar activa={activa} tipo="fiados" rango={rango} />
      </div>
      <div className="grid gap-3 lg:grid-cols-2">
        <Panel titulo="Antigüedad de la deuda (días)">
          <Barras
            filas={r.antiguedad.map((t) => ({ etiqueta: `${t.tramo} días`, valor: t.monto, detalle: `${pesos(t.monto)} en ${t.ventas} ventas` }))}
            formato={pesos}
          />
        </Panel>
        <Panel titulo="Los que más debían al cierre">
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
