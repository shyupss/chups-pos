// Gráficos hechos a mano en SVG, con los tokens de color (se leen bien en claro y oscuro). Sin
// librerías de gráficos: cuidan el peso del instalador. Al pasar el mouse se ve el valor exacto.

import { useEffect, useRef, useState, type ReactNode } from "react";

/** Ancho real del contenedor, para dibujar el SVG a su tamaño (sin estirar el texto). */
function useAncho<T extends HTMLElement>(): [React.RefObject<T | null>, number] {
  const ref = useRef<T>(null);
  const [ancho, setAncho] = useState(0);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const obs = new ResizeObserver(([e]) => setAncho(Math.floor(e.contentRect.width)));
    obs.observe(el);
    return () => obs.disconnect();
  }, []);
  return [ref, ancho];
}

/** Recuadro con el valor exacto, sobre el punto señalado. */
function Globo({ x, y, children }: { x: number; y: number; children: ReactNode }) {
  return (
    <div
      role="tooltip"
      className="pointer-events-none absolute z-10 -translate-x-1/2 -translate-y-full rounded-md border border-linea bg-superficie px-2 py-1 text-xs whitespace-nowrap shadow-s2 tabular-nums"
      style={{ left: x, top: y - 6 }}
    >
      {children}
    </div>
  );
}

export interface Columna {
  /** Bajo la columna (se muestra cada `etiquetaCada`). */
  etiqueta: string;
  valor: number;
  /** El texto del globo: lo exacto de esa columna. */
  detalle: ReactNode;
}

/** Columnas verticales (ventas por hora o por día). */
export function Columnas({
  columnas,
  alto = 180,
  etiquetaCada = 1,
  formatoEje,
  descripcion,
}: {
  columnas: Columna[];
  alto?: number;
  etiquetaCada?: number;
  /** Cómo escribir el máximo en el eje ($1,2 M). */
  formatoEje: (n: number) => string;
  /** Para lectores de pantalla. */
  descripcion: string;
}) {
  const [ref, ancho] = useAncho<HTMLDivElement>();
  const [marcada, setMarcada] = useState<number | null>(null);
  const max = Math.max(1, ...columnas.map((c) => c.valor));
  const izq = 48;
  const abajo = 20;
  const util = Math.max(0, ancho - izq);
  const paso = columnas.length > 0 ? util / columnas.length : 0;
  const barra = Math.max(1, paso * 0.72);
  const altoUtil = alto - abajo - 8;
  const y = (v: number) => 8 + altoUtil - (v / max) * altoUtil;

  return (
    <div ref={ref} className="relative w-full" onMouseLeave={() => setMarcada(null)}>
      {ancho > 0 && (
        <svg width={ancho} height={alto} role="img" aria-label={descripcion} className="block">
          {[0, 0.5, 1].map((f) => (
            <g key={f}>
              <line x1={izq} x2={ancho} y1={y(max * f)} y2={y(max * f)} className="stroke-linea" />
              <text x={izq - 6} y={y(max * f) + 4} textAnchor="end" className="fill-tenue text-[10px]">
                {formatoEje(Math.round(max * f))}
              </text>
            </g>
          ))}
          {columnas.map((c, i) => {
            const x = izq + i * paso + (paso - barra) / 2;
            const h = 8 + altoUtil - y(c.valor);
            return (
              <g key={i} onMouseEnter={() => setMarcada(i)}>
                {/* Zona invisible de toda la altura: se puede señalar también una columna en cero. */}
                <rect x={izq + i * paso} y={0} width={paso} height={alto - abajo} fill="transparent" />
                <rect
                  x={x}
                  y={y(c.valor)}
                  width={barra}
                  height={Math.max(0, h)}
                  rx={Math.min(3, barra / 3)}
                  className={marcada === i ? "fill-texto" : "fill-acento"}
                />
                {i % etiquetaCada === 0 && (
                  <text x={x + barra / 2} y={alto - 6} textAnchor="middle" className="fill-tenue text-[10px]">
                    {c.etiqueta}
                  </text>
                )}
              </g>
            );
          })}
        </svg>
      )}
      {marcada !== null && columnas[marcada] && (
        <Globo x={izq + marcada * paso + paso / 2} y={y(columnas[marcada].valor)}>
          {columnas[marcada].detalle}
        </Globo>
      )}
    </div>
  );
}

/** Barras horizontales con el valor escrito (medios de pago, categorías, días de la semana). */
export function Barras({
  filas,
  formato,
}: {
  filas: { etiqueta: string; valor: number; detalle?: string }[];
  formato: (n: number) => string;
}) {
  const max = Math.max(1, ...filas.map((f) => f.valor));
  if (filas.length === 0) return <p className="text-tenue">Sin datos</p>;
  return (
    <ul className="space-y-2">
      {filas.map((f) => (
        <li key={f.etiqueta} title={f.detalle ?? formato(f.valor)}>
          <div className="flex justify-between gap-3 text-sm">
            <span className="min-w-0 truncate">{f.etiqueta}</span>
            <span className="shrink-0 font-medium tabular-nums">{formato(f.valor)}</span>
          </div>
          <svg width="100%" height="8" className="mt-1 block" aria-hidden>
            <rect width="100%" height="8" rx="4" className="fill-linea" />
            <rect width={`${(Math.max(0, f.valor) / max) * 100}%`} height="8" rx="4" className="fill-acento" />
          </svg>
        </li>
      ))}
    </ul>
  );
}

const DIAS = ["Lunes", "Martes", "Miércoles", "Jueves", "Viernes", "Sábado", "Domingo"];

/** Mapa de calor día de la semana × hora: más intenso, más vendido. */
export function MapaCalor({
  celdas,
  valor,
  detalle,
}: {
  /** 7 filas (lunes primero) × 24 horas. */
  celdas: { monto: number; boletas: number }[][];
  valor: (c: { monto: number; boletas: number }) => number;
  detalle: (c: { monto: number; boletas: number }) => string;
}) {
  const [ref, ancho] = useAncho<HTMLDivElement>();
  const [marcada, setMarcada] = useState<[number, number] | null>(null);
  const max = Math.max(1, ...celdas.flat().map(valor));
  const izq = 34;
  const arriba = 16;
  const lado = Math.max(8, Math.min(28, (ancho - izq) / 24));
  const alto = arriba + lado * 7;
  return (
    <div ref={ref} className="relative w-full" onMouseLeave={() => setMarcada(null)}>
      {ancho > 0 && (
        <svg width={izq + lado * 24} height={alto} role="img" aria-label="Ventas por día de la semana y hora" className="block">
          {Array.from({ length: 24 }, (_, h) =>
            h % 3 === 0 ? (
              <text key={h} x={izq + h * lado + lado / 2} y={11} textAnchor="middle" className="fill-tenue text-[10px]">
                {h}
              </text>
            ) : null,
          )}
          {celdas.map((fila, d) => (
            <g key={d}>
              <text x={izq - 6} y={arriba + d * lado + lado / 2 + 4} textAnchor="end" className="fill-tenue text-[10px]">
                {DIAS[d].slice(0, 2)}
              </text>
              {fila.map((c, h) => {
                const v = valor(c);
                return (
                  <rect
                    key={h}
                    x={izq + h * lado + 1}
                    y={arriba + d * lado + 1}
                    width={lado - 2}
                    height={lado - 2}
                    rx={3}
                    className={v > 0 ? "fill-acento" : "fill-linea"}
                    fillOpacity={v > 0 ? 0.12 + 0.88 * (v / max) : 0.6}
                    stroke={marcada?.[0] === d && marcada[1] === h ? "currentColor" : "none"}
                    onMouseEnter={() => setMarcada([d, h])}
                  />
                );
              })}
            </g>
          ))}
        </svg>
      )}
      {marcada && (
        <Globo x={izq + marcada[1] * lado + lado / 2} y={arriba + marcada[0] * lado}>
          {DIAS[marcada[0]]} {marcada[1]}:00–{marcada[1] + 1}:00 · {detalle(celdas[marcada[0]][marcada[1]])}
        </Globo>
      )}
    </div>
  );
}

export { DIAS };
