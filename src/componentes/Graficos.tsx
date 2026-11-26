// Gráficos hechos a mano en SVG, con los tokens de color (se leen bien en claro y oscuro). Sin
// librerías de gráficos: cuidan el peso del instalador. Al pasar el mouse se ve el valor exacto.

import { useEffect, useRef, useState, type ReactNode } from "react";
import { escalaY, indiceMasCercano, polilinea, puntosDeSerie } from "../pantallas/reportes/reportes-nucleo";

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
  // Las etiquetas miden 13 px (el mínimo de la interfaz): más margen para el eje y para el pie.
  const izq = 60;
  const abajo = 26;
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
              <text x={izq - 6} y={y(max * f) + 4.5} textAnchor="end" className="fill-tenue text-[13px]">
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
                  <text x={x + barra / 2} y={alto - 7} textAnchor="middle" className="fill-tenue text-[13px]">
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
  const izq = 36;
  const arriba = 22;
  const lado = Math.max(8, Math.min(28, (ancho - izq) / 24));
  const alto = arriba + lado * 7;
  return (
    <div ref={ref} className="relative w-full" onMouseLeave={() => setMarcada(null)}>
      {ancho > 0 && (
        <svg width={izq + lado * 24} height={alto} role="img" aria-label="Ventas por día de la semana y hora" className="block">
          {Array.from({ length: 24 }, (_, h) =>
            h % 3 === 0 ? (
              <text key={h} x={izq + h * lado + lado / 2} y={15} textAnchor="middle" className="fill-tenue text-[13px]">
                {h}
              </text>
            ) : null,
          )}
          {celdas.map((fila, d) => (
            <g key={d}>
              <text x={izq - 6} y={arriba + d * lado + lado / 2 + 4.5} textAnchor="end" className="fill-tenue text-[13px]">
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

export interface SerieLinea {
  nombre: string;
  /** Un valor por posición del eje X; una serie puede ser más corta que el eje (el mes que va). */
  valores: number[];
  /** Línea punteada, para lo que se compara (el mes pasado). */
  punteada?: boolean;
}

/**
 * Gráfico de líneas (ventas por día): una polyline por serie, rayas guía suaves, el eje Y en pesos cortos
 * ($120 mil) y, al pasar el mouse por un día, un punto en cada línea con el día y el monto. SVG propio.
 */
export function Lineas({
  series,
  etiquetas,
  titulos,
  formatoEje,
  formatoValor,
  descripcion,
  alto = 240,
}: {
  series: SerieLinea[];
  /** El texto bajo cada posición del eje X ("1", "2"… o "3 nov"); se muestran los que caben. */
  etiquetas: string[];
  /** El encabezado del recuadro de cada posición: "Martes 3 de noviembre". */
  titulos: string[];
  formatoEje: (n: number) => string;
  formatoValor: (n: number) => string;
  /** Para lectores de pantalla. */
  descripcion: string;
  alto?: number;
}) {
  const [ref, ancho] = useAncho<HTMLDivElement>();
  const [marcada, setMarcada] = useState<number | null>(null);
  const izq = 64;
  const der = 14;
  const arriba = 12;
  const abajo = 28;
  const w = Math.max(0, ancho - izq - der);
  const h = alto - arriba - abajo;
  const largoX = etiquetas.length;
  const max = Math.max(0, ...series.flatMap((s) => s.valores));
  const { tope, valores: marcas } = escalaY(max);
  const puntos = series.map((s) => puntosDeSerie(s.valores, w, h, tope, largoX));
  const cada = Math.max(1, Math.ceil(largoX / Math.max(1, Math.floor(w / 46))));
  const globo = marcada === null ? null : puntos.map((p) => p[marcada]).filter(Boolean);

  return (
    <div>
      <div ref={ref} className="relative w-full" onMouseLeave={() => setMarcada(null)}>
        {ancho > 0 && (
          <svg
            width={ancho}
            height={alto}
            role="img"
            aria-label={descripcion}
            className="block"
            onMouseMove={(e) => {
              const caja = e.currentTarget.getBoundingClientRect();
              const x = e.clientX - caja.left - izq;
              setMarcada(x < -izq / 2 || x > w + der ? null : indiceMasCercano(x, w, largoX));
            }}
          >
            {marcas.map((v) => {
              const y = arriba + h - (v / tope) * h;
              return (
                <g key={v}>
                  <line x1={izq} x2={ancho - der} y1={y} y2={y} className="stroke-linea" />
                  <text x={izq - 8} y={y + 4.5} textAnchor="end" className="fill-tenue text-[13px]">
                    {formatoEje(v)}
                  </text>
                </g>
              );
            })}
            {etiquetas.map((t, i) =>
              i % cada === 0 ? (
                <text
                  key={i}
                  x={izq + (largoX <= 1 ? w / 2 : (i / (largoX - 1)) * w)}
                  y={alto - 6}
                  textAnchor="middle"
                  className="fill-tenue text-[13px]"
                >
                  {t}
                </text>
              ) : null,
            )}
            {marcada !== null && largoX > 1 && (
              <line x1={izq + (marcada / (largoX - 1)) * w} x2={izq + (marcada / (largoX - 1)) * w} y1={arriba} y2={arriba + h} className="stroke-borde-control" />
            )}
            {series.map((s, i) => (
              <polyline
                key={s.nombre}
                points={polilinea(puntos[i].map((p) => ({ x: p.x + izq, y: p.y + arriba })))}
                fill="none"
                strokeWidth={s.punteada ? 2 : 3}
                strokeLinejoin="round"
                strokeLinecap="round"
                strokeDasharray={s.punteada ? "6 6" : undefined}
                className={s.punteada ? "stroke-tenue" : "stroke-acento"}
              />
            ))}
            {/* Con un solo día no hay línea que dibujar: se ve el punto. */}
            {largoX === 1 &&
              puntos.map((p, i) => p[0] && <circle key={i} cx={p[0].x + izq} cy={p[0].y + arriba} r={4} className={series[i].punteada ? "fill-tenue" : "fill-acento"} />)}
            {marcada !== null &&
              puntos.map((p, i) =>
                p[marcada] ? (
                  <circle
                    key={series[i].nombre}
                    cx={p[marcada].x + izq}
                    cy={p[marcada].y + arriba}
                    r={5}
                    strokeWidth={2}
                    className={`stroke-superficie ${series[i].punteada ? "fill-tenue" : "fill-acento"}`}
                  />
                ) : null,
              )}
          </svg>
        )}
        {marcada !== null && globo && globo.length > 0 && (
          <Globo x={izq + (largoX <= 1 ? w / 2 : (marcada / (largoX - 1)) * w)} y={arriba + Math.min(...globo.map((p) => p.y))}>
            <div className="font-medium">{titulos[marcada]}</div>
            {series.map((s) =>
              s.valores[marcada] === undefined ? null : (
                <div key={s.nombre} className={s.punteada ? "text-tenue" : ""}>
                  {s.nombre}: {formatoValor(s.valores[marcada])}
                </div>
              ),
            )}
          </Globo>
        )}
      </div>
      {series.length > 1 && (
        <ul className="mt-2 flex flex-wrap gap-x-5 gap-y-1 text-sm text-tenue">
          {series.map((s) => (
            <li key={s.nombre} className="flex items-center gap-2">
              <svg width="26" height="8" aria-hidden>
                <line x1="1" x2="25" y1="4" y2="4" strokeWidth={s.punteada ? 2 : 3} strokeLinecap="round" strokeDasharray={s.punteada ? "5 5" : undefined} className={s.punteada ? "stroke-tenue" : "stroke-acento"} />
              </svg>
              {s.nombre}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
