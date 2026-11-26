import { useMemo } from "react";
import { api } from "../../api";
import { Barras } from "../../componentes/Graficos";
import { NOMBRE_MEDIO, pesos } from "../../formato";
import { Panel, Tarjeta } from "./Hoy";
import { AYUDA, GraficoDias } from "./PorRango";
import { Exportar, useReporte } from "./Rango";
import { completarDias, periodoAnterior, type Rango } from "./reportes-nucleo";

/**
 * Reportes > Resumen: cómo va el mes (o el período que se elija) de una mirada. Cuatro cifras grandes, las ventas
 * de cada día contra el período anterior (línea punteada), con qué pagan los clientes y lo que más se vende.
 * El detalle está en las demás pestañas.
 */
export default function Resumen({
  activa,
  rango,
  onVerProductos,
}: {
  activa: boolean;
  rango: Rango;
  /** Lleva a la pestaña Productos, con la lista completa. */
  onVerProductos: () => void;
}) {
  const anterior = useMemo(() => periodoAnterior(rango), [rango]);
  const { datos: v, error } = useReporte(() => api.reporteVentas(rango.desde, rango.hasta), [rango.desde, rango.hasta]);
  const { datos: va } = useReporte(() => api.reporteVentas(anterior.desde, anterior.hasta), [anterior.desde, anterior.hasta]);
  const { datos: g } = useReporte(() => api.reporteGanancia(rango.desde, rango.hasta), [rango.desde, rango.hasta]);
  const { datos: p } = useReporte(() => api.reporteProductos(rango.desde, rango.hasta, null), [rango.desde, rango.hasta]);

  const dias = useMemo(() => (v ? completarDias(v.porDia, rango.desde, rango.hasta) : []), [v, rango.desde, rango.hasta]);
  const anteriores = useMemo(() => (va ? completarDias(va.porDia, anterior.desde, anterior.hasta) : undefined), [va, anterior.desde, anterior.hasta]);
  const masVendidos = useMemo(() => [...(p?.productos ?? [])].sort((a, b) => b.monto - a.monto).slice(0, 5), [p]);

  if (!v) {
    return error ? <p className="rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p> : <p className="text-tenue">Cargando…</p>;
  }
  const boletas = (n: number) => (n === 1 ? "1 boleta" : `${n} boletas`);

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="grid flex-1 grid-cols-2 gap-3 lg:grid-cols-4">
          <Tarjeta titulo="Vendido" valor={pesos(v.total)} pie="Sin las anuladas" ayuda={AYUDA.vendido} />
          <Tarjeta
            titulo="Ganancia"
            valor={g ? pesos(g.ganancia) : "…"}
            pie={g && g.ventasSinCosto > 0 ? `Sin contar ${pesos(g.ventasSinCosto)} vendidos sin costo` : "Ventas menos costo"}
            ayuda={AYUDA.ganancia}
          />
          <Tarjeta titulo="Ventas" valor={v.boletas.toLocaleString("es-CL")} pie="Boletas del período" ayuda={AYUDA.ventas} />
          <Tarjeta titulo="Gasto promedio por venta" valor={pesos(v.ticketPromedio)} ayuda={AYUDA.ticket} />
        </div>
        <Exportar activa={activa} tipo="ventas" rango={rango} nombre="Ventas" />
      </div>

      <Panel titulo="¿Cuánto vendí cada día?">
        <GraficoDias
          dias={dias}
          anteriores={anteriores}
          nombreActual={rango.atajo === "mes" ? "Este mes" : undefined}
          nombreAnterior={anterior.nombre}
        />
      </Panel>

      <div className="grid gap-3 lg:grid-cols-2">
        <Panel titulo="¿Con qué me pagan?">
          <Barras
            filas={v.porMedio.map((m) => ({ etiqueta: NOMBRE_MEDIO[m.medio], valor: m.monto, detalle: `${pesos(m.monto)} · ${boletas(m.boletas)}` }))}
            formato={pesos}
          />
        </Panel>
        <Panel titulo="¿Qué se vende más?" accion={masVendidos.length > 0 ? { texto: "Ver todos", alHacer: onVerProductos } : undefined}>
          {p === null ? (
            <p className="text-tenue">Cargando…</p>
          ) : (
            <Barras filas={masVendidos.map((x) => ({ etiqueta: x.nombre, valor: x.monto }))} formato={pesos} />
          )}
        </Panel>
      </div>
    </div>
  );
}
