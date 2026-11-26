// Reglas de los reportes, sin React: los rangos de fechas, el período dicho en palabras y la matemática del
// gráfico de líneas. Se prueban con `npm test`.

// La extensión .ts hace falta para que `npm test` (Node) encuentre el módulo.
import { diaLocal } from "../../formato.ts";

export type Atajo = "hoy" | "ayer" | "semana" | "mes" | "mesPasado" | "ultimos30" | "anio" | "personalizado";

export interface Rango {
  atajo: Atajo;
  /** "aaaa-mm-dd" locales, ambos incluidos. */
  desde: string;
  hasta: string;
}

/** Las fechas de un atajo, en el calendario local (la semana empieza el lunes). */
export function rangoDe(atajo: Atajo, hoy = new Date()): Rango {
  const d = (a: number, m: number, dia: number) => diaLocal(new Date(a, m, dia));
  const [a, m, dia] = [hoy.getFullYear(), hoy.getMonth(), hoy.getDate()];
  const deHoy = diaLocal(hoy);
  switch (atajo) {
    case "ayer":
      return { atajo, desde: d(a, m, dia - 1), hasta: d(a, m, dia - 1) };
    case "semana":
      return { atajo, desde: d(a, m, dia - ((hoy.getDay() + 6) % 7)), hasta: deHoy };
    case "mes":
      return { atajo, desde: d(a, m, 1), hasta: deHoy };
    case "mesPasado":
      return { atajo, desde: d(a, m - 1, 1), hasta: d(a, m, 0) };
    case "ultimos30":
      return { atajo, desde: d(a, m, dia - 29), hasta: deHoy };
    case "anio":
      return { atajo, desde: d(a, 0, 1), hasta: deHoy };
    default:
      return { atajo, desde: deHoy, hasta: deHoy };
  }
}

const MESES = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];
const DIAS_SEMANA = ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"];

/** "aaaa-mm-dd" como fecha local a mediodía (para no correr de día con el huso horario). */
export function aFecha(texto: string): Date {
  const [a, m, d] = texto.split("-").map(Number);
  return new Date(a, m - 1, d, 12);
}

const mayuscula = (t: string) => t.charAt(0).toUpperCase() + t.slice(1);

/**
 * El período en palabras para el título de un reporte: "Noviembre 2026" (el mes entero o el mes que va),
 * "Del 3 al 17 de noviembre", "Hoy, jueves 26 de noviembre". Nunca fechas con guiones.
 */
export function periodoEnPalabras(desde: string, hasta: string, hoy = new Date()): string {
  const a = aFecha(desde);
  const b = aFecha(hasta);
  const deHoy = diaLocal(hoy);
  const mismoMes = a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth();
  const anioActual = hoy.getFullYear();

  if (desde === hasta) {
    const anio = a.getFullYear() === anioActual ? "" : ` de ${a.getFullYear()}`;
    const dia = `${DIAS_SEMANA[a.getDay()]} ${a.getDate()} de ${MESES[a.getMonth()]}${anio}`;
    return desde === deHoy ? `Hoy, ${dia}` : mayuscula(dia);
  }
  if (mismoMes && a.getDate() === 1) {
    const ultimo = new Date(a.getFullYear(), a.getMonth() + 1, 0).getDate();
    // El mes entero, o el mes que va (hasta hoy): se dice solo el mes.
    if (b.getDate() === ultimo || hasta === deHoy) return `${mayuscula(MESES[a.getMonth()])} ${a.getFullYear()}`;
  }
  if (mismoMes) {
    const anio = a.getFullYear() === anioActual ? "" : ` de ${a.getFullYear()}`;
    return `Del ${a.getDate()} al ${b.getDate()} de ${MESES[a.getMonth()]}${anio}`;
  }
  if (a.getFullYear() === b.getFullYear()) {
    const anio = a.getFullYear() === anioActual ? "" : ` de ${a.getFullYear()}`;
    return `Del ${a.getDate()} de ${MESES[a.getMonth()]} al ${b.getDate()} de ${MESES[b.getMonth()]}${anio}`;
  }
  return `Del ${a.getDate()} de ${MESES[a.getMonth()]} de ${a.getFullYear()} al ${b.getDate()} de ${MESES[b.getMonth()]} de ${b.getFullYear()}`;
}

/** Cuántos días tiene el rango, ambos incluidos. */
export function diasDelRango(desde: string, hasta: string): number {
  return Math.round((aFecha(hasta).getTime() - aFecha(desde).getTime()) / 86_400_000) + 1;
}

/**
 * Con qué se compara un período: un mes (entero o el que va) con el mes anterior completo; cualquier otro rango
 * con el mismo número de días justo antes.
 */
export function periodoAnterior(r: Rango): { desde: string; hasta: string; nombre: string } {
  const a = aFecha(r.desde);
  const esMes = a.getDate() === 1 && (r.atajo === "mes" || r.atajo === "mesPasado");
  if (esMes) {
    const desde = diaLocal(new Date(a.getFullYear(), a.getMonth() - 1, 1));
    const hasta = diaLocal(new Date(a.getFullYear(), a.getMonth(), 0));
    return { desde, hasta, nombre: "Mes pasado" };
  }
  const n = diasDelRango(r.desde, r.hasta);
  const hasta = diaLocal(new Date(a.getFullYear(), a.getMonth(), a.getDate() - 1));
  const desde = diaLocal(new Date(a.getFullYear(), a.getMonth(), a.getDate() - n));
  return { desde, hasta, nombre: "Período anterior" };
}

export interface VentaDia {
  dia: string;
  monto: number;
  boletas: number;
}

/** Un valor por cada día del rango: los días sin ventas valen 0 (la línea no se salta días). */
export function completarDias(porDia: readonly VentaDia[], desde: string, hasta: string): VentaDia[] {
  const mapa = new Map(porDia.map((d) => [d.dia, d]));
  const n = Math.max(0, diasDelRango(desde, hasta));
  const a = aFecha(desde);
  return Array.from({ length: n }, (_, i) => {
    const dia = diaLocal(new Date(a.getFullYear(), a.getMonth(), a.getDate() + i));
    return mapa.get(dia) ?? { dia, monto: 0, boletas: 0 };
  });
}

/**
 * El eje Y con valores "redondos": hasta dónde llega y las marcas (0, 25, 50…). Con todo en cero igual se
 * dibuja un eje (de 0 a 4.000) en vez de dividir por cero.
 */
export function escalaY(max: number, marcas = 4): { tope: number; valores: number[] } {
  const maximo = Number.isFinite(max) && max > 0 ? max : 0;
  const crudo = maximo === 0 ? 1000 : maximo / marcas;
  const magnitud = 10 ** Math.floor(Math.log10(crudo));
  const paso = [1, 1.5, 2, 2.5, 3, 4, 5, 10].map((f) => f * magnitud).find((c) => c >= crudo) ?? 10 * magnitud;
  return { tope: paso * marcas, valores: Array.from({ length: marcas + 1 }, (_, i) => paso * i) };
}

export interface Punto {
  x: number;
  y: number;
}

/**
 * Los puntos de una serie en un área de `ancho` × `alto`. El eje X tiene `largoX` posiciones (puede ser más que
 * los valores: el mes que va tiene menos días que el pasado). Con una sola posición el punto va al centro.
 */
export function puntosDeSerie(valores: readonly number[], ancho: number, alto: number, tope: number, largoX: number): Punto[] {
  const x = (i: number) => (largoX <= 1 ? ancho / 2 : (i / (largoX - 1)) * ancho);
  const y = (v: number) => alto - Math.max(0, Math.min(1, tope > 0 ? v / tope : 0)) * alto;
  return valores.map((v, i) => ({ x: x(i), y: y(v) }));
}

/** El texto de un `<polyline points>`. */
export function polilinea(puntos: readonly Punto[]): string {
  return puntos.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(" ");
}

/** La posición del eje X más cercana a un punto del mouse (para mostrar el valor de ese día). */
export function indiceMasCercano(x: number, ancho: number, largoX: number): number {
  if (largoX <= 1) return 0;
  return Math.max(0, Math.min(largoX - 1, Math.round((x / ancho) * (largoX - 1))));
}
