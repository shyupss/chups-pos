import Boton from "../../componentes/Boton";
import { ArrowDown, ArrowUp } from "lucide-react";
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { api, escucharCambiosDelDia, textoError, type ResumenHoy } from "../../api";
import { Barras, Columnas } from "../../componentes/Graficos";
import InfoAyuda from "../../componentes/InfoAyuda";
import { cantidad, NOMBRE_MEDIO, pesos } from "../../formato";
import { escucharCambioStock } from "../../eventos";
import { pesosCortos, porcentaje } from "./formato";

interface Props {
  activa: boolean;
  onVerBajoMinimo: () => void;
  onVerCliente: (id: number) => void;
  onVerProducto: (p: { id: number; nombre: string }) => void;
}

const DIA_LARGO = new Intl.DateTimeFormat("es-CL", { weekday: "long", day: "numeric", month: "long" });
const DIA_SEMANA = new Intl.DateTimeFormat("es-CL", { weekday: "long" });

/** "aaaa-mm-dd" como fecha local a mediodía (para no correr de día con el huso). */
const dia = (texto: string) => new Date(`${texto}T12:00:00`);

/** Reportes > Hoy: cómo va el día. Se recarga al volver a la pantalla y cuando Rust avisa un cambio. */
export default function Hoy({ activa, onVerBajoMinimo, onVerCliente, onVerProducto }: Props) {
  const [hoy, setHoy] = useState<ResumenHoy | null>(null);
  const [error, setError] = useState<string | null>(null);

  const cargar = useCallback(() => {
    api
      .reporteHoy()
      .then((h) => {
        setHoy(h);
        setError(null);
      })
      .catch((e) => setError(textoError(e)));
  }, []);

  useEffect(() => {
    if (!activa) return;
    cargar();
    const dejarRust = escucharCambiosDelDia(cargar);
    const dejarStock = escucharCambioStock(cargar);
    return () => {
      dejarRust();
      dejarStock();
    };
  }, [activa, cargar]);

  if (error) return <p className="rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>;
  if (!hoy) return <p className="text-tenue">Cargando…</p>;

  const semanaPasada = DIA_SEMANA.format(dia(hoy.fecha));
  const sube = (hoy.variacionPb ?? 0) >= 0;

  return (
    <div className="space-y-4">
      <p className="text-tenue first-letter:uppercase">{DIA_LARGO.format(dia(hoy.fecha))}</p>

      <div className="grid grid-cols-2 gap-3 xl:grid-cols-4">
        <Tarjeta
          titulo="Ventas de hoy"
          valor={pesos(hoy.ventas)}
          pie={hoy.boletas === 1 ? "1 boleta" : `${hoy.boletas} boletas`}
          ayuda="Todo lo que vendiste hoy, sin contar las ventas anuladas y descontando las devoluciones."
        />
        <Tarjeta
          titulo="Gasto promedio por venta"
          valor={pesos(hoy.ticketPromedio)}
          ayuda="Lo que gasta en promedio cada cliente por compra: lo vendido hoy dividido por la cantidad de ventas. No cuenta ventas anuladas y descuenta las devoluciones. También se le dice «ticket promedio»."
        />
        <Tarjeta
          titulo="Ganancia de hoy"
          ayuda="Lo vendido menos lo que te costaron esos productos. Solo cuenta productos con costo registrado."
          valor={pesos(hoy.ganancia)}
          pie={hoy.ventasSinCosto > 0 ? `Sin contar ${pesos(hoy.ventasSinCosto)} vendidos sin costo` : "Ventas menos costo"}
        />
        <Tarjeta
          titulo={`Vs. el ${semanaPasada} pasado`}
          valor={
            hoy.variacionPb === null ? (
              "—"
            ) : (
              <span className={`flex items-center gap-1 ${sube ? "text-exito" : "text-peligro"}`}>
                {sube ? <ArrowUp size={22} aria-hidden /> : <ArrowDown size={22} aria-hidden />}
                {porcentaje(Math.abs(hoy.variacionPb))}
                <span className="sr-only">{sube ? "más" : "menos"}</span>
              </span>
            )
          }
          pie={`A esta hora llevaba ${pesos(hoy.ventasSemanaAnterior)}`}
        />
        <Tarjeta
          titulo="Plata que debería haber en la caja"
          ayuda="Lo que dejaste al abrir, más lo cobrado en efectivo, más los ingresos, menos los retiros del turno abierto."
          valor={hoy.efectivoEsperado === null ? "Caja cerrada" : pesos(hoy.efectivoEsperado)}
          pie={hoy.efectivoEsperado === null ? "Abre la caja para vender" : "En el turno abierto"}
        />
        <Tarjeta titulo="Fiado otorgado hoy" valor={pesos(hoy.fiadoOtorgado)} pie="Ventas fiadas del día" ayuda="Lo que vendiste fiado hoy: todavía no lo cobras." />
        <Tarjeta titulo="Pagos de fiado hoy" valor={pesos(hoy.pagosFiado)} pie="Lo que pagaron los clientes" ayuda="Lo que los clientes te pagaron hoy de sus deudas." />
      </div>

      <div className="grid gap-3 lg:grid-cols-3">
        <Panel titulo="¿A qué hora vendí?" className="lg:col-span-2">
          <Columnas
            descripcion="Ventas de hoy por hora"
            columnas={hoy.porHora.map((c, h) => ({
              etiqueta: String(h),
              valor: c.monto,
              detalle: `${h}:00–${h + 1}:00 · ${pesos(c.monto)} · ${c.boletas} ${c.boletas === 1 ? "boleta" : "boletas"}`,
            }))}
            etiquetaCada={2}
            formatoEje={pesosCortos}
          />
        </Panel>
        <Panel titulo="¿Con qué me pagan?">
          <Barras
            filas={hoy.porMedio.map((m) => ({
              etiqueta: NOMBRE_MEDIO[m.medio],
              valor: m.monto,
              detalle: `${pesos(m.monto)} en ${m.boletas} ${m.boletas === 1 ? "boleta" : "boletas"}`,
            }))}
            formato={pesos}
          />
        </Panel>
      </div>

      <div className="grid gap-3 lg:grid-cols-3">
        <Panel titulo="¿Qué se vendió más hoy?">
          <Lista vacia="Todavía no hay ventas hoy.">
            {hoy.masVendidos.map((p) => (
              <li key={p.productoId} className="flex items-baseline gap-3">
                <span className="min-w-0 flex-1 truncate">
                  <Boton variante="enlace" onClick={() => onVerProducto({ id: p.productoId, nombre: p.nombre })} className="whitespace-nowrap" title="Ver movimientos">
                    {p.nombre}
                  </Boton>
                </span>
                <span className="text-tenue tabular-nums">{cantidad(p.cantidadMil, p.unidad)}</span>
                <span className="w-20 text-right font-medium tabular-nums">{pesos(p.monto)}</span>
              </li>
            ))}
          </Lista>
        </Panel>
        <Panel
          titulo={`Bajo el mínimo${hoy.totalBajoMinimo > 0 ? ` (${hoy.totalBajoMinimo})` : ""}`}
          accion={hoy.totalBajoMinimo > 0 ? { texto: "Ver en Inventario", alHacer: onVerBajoMinimo } : undefined}
        >
          <Lista vacia="Ningún producto bajo su mínimo.">
            {hoy.bajoMinimo.map((p) => (
              <li key={p.id} className="flex items-baseline gap-3">
                <span className="min-w-0 flex-1 truncate">{p.nombre}</span>
                <span className="font-medium text-peligro tabular-nums">{cantidad(p.stockMil, p.unidad)}</span>
                <span className="text-xs text-tenue tabular-nums">mín. {cantidad(p.stockMinimoMil, p.unidad)}</span>
              </li>
            ))}
          </Lista>
        </Panel>
        <Panel titulo={`Fiado atrasado (más de ${hoy.diasAlerta} días)`}>
          <Lista vacia="Nadie tiene deudas atrasadas.">
            {hoy.atrasados.map((c) => (
              <li key={c.id} className="flex items-baseline gap-3">
                <span className="min-w-0 flex-1 truncate">
                  <Boton variante="enlace" onClick={() => onVerCliente(c.id)} className="whitespace-nowrap">
                    {c.nombre}
                  </Boton>
                </span>
                <span className="rounded bg-aviso-fondo px-1.5 text-xs font-medium text-aviso">{c.dias} días</span>
                <span className="w-20 text-right font-medium tabular-nums">{pesos(c.deuda)}</span>
              </li>
            ))}
          </Lista>
        </Panel>
      </div>
    </div>
  );
}

/** Una cifra grande con su título; `ayuda` agrega el ícono (i) con lo que mide, en una frase. */
export function Tarjeta({ titulo, valor, pie, ayuda }: { titulo: string; valor: ReactNode; pie?: string; ayuda?: string }) {
  return (
    <div className="tarjeta p-4">
      <div className="flex items-start justify-between gap-2">
        <div className="text-xs font-medium text-tenue">{titulo}</div>
        {ayuda && <InfoAyuda texto={ayuda} etiqueta={titulo} />}
      </div>
      <div className="mt-1 text-2xl font-semibold tabular-nums">{valor}</div>
      {pie && (
        <div className="mt-1 truncate text-xs text-tenue" title={pie}>
          {pie}
        </div>
      )}
    </div>
  );
}

export function Panel({
  titulo,
  children,
  className = "",
  accion,
}: {
  titulo: string;
  children: ReactNode;
  className?: string;
  accion?: { texto: string; alHacer: () => void };
}) {
  return (
    <section className={`tarjeta p-4 ${className}`}>
      <div className="mb-3 flex items-baseline justify-between gap-3">
        <h2 className="font-semibold">{titulo}</h2>
        {accion && (
          <Boton variante="enlace" onClick={accion.alHacer} className="text-sm">
            {accion.texto}
          </Boton>
        )}
      </div>
      {children}
    </section>
  );
}

function Lista({ children, vacia }: { children: ReactNode[]; vacia: string }) {
  if (children.length === 0) return <p className="text-tenue">{vacia}</p>;
  return <ul className="space-y-2">{children}</ul>;
}
