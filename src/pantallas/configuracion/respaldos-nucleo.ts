// Los textos del estado de las copias de seguridad, sin React: se prueban con `npm test`.

/** Desde cuántos días sin copia se avisa con color de aviso en vez de verde. */
export const DIAS_AVISO_COPIA = 3;

const dos = (n: number) => String(n).padStart(2, "0");
const horaDe = (f: Date) => `${dos(f.getHours())}:${dos(f.getMinutes())}`;
const inicioDelDia = (f: Date) => new Date(f.getFullYear(), f.getMonth(), f.getDate()).getTime();

/** Cuántos días de calendario pasaron entre dos momentos (hoy = 0, ayer = 1). */
export function diasDeCalendario(desde: Date, hasta: Date): number {
  return Math.round((inicioDelDia(hasta) - inicioDelDia(desde)) / 86_400_000);
}

/**
 * "Tu última copia es de hoy a las 13:05" (verde) o, si pasaron más de 3 días, "…de hace 5 días, el 21-11-2026 a las
 * 10:12" en aviso. Sin ninguna copia, dice que todavía no hay y que conviene hacerla.
 */
export function estadoDeLaCopia(ultima: string | null, ahora = new Date()): { texto: string; viejo: boolean } {
  if (!ultima) return { texto: "Todavía no hay copias de seguridad: conviene hacer una ahora.", viejo: true };
  const f = new Date(ultima);
  const dias = diasDeCalendario(f, ahora);
  const hora = `a las ${horaDe(f)}`;
  if (dias <= 0) return { texto: `Tu última copia es de hoy ${hora}`, viejo: false };
  if (dias === 1) return { texto: `Tu última copia es de ayer ${hora}`, viejo: false };
  const fecha = `${dos(f.getDate())}-${dos(f.getMonth() + 1)}-${f.getFullYear()}`;
  if (dias <= DIAS_AVISO_COPIA) return { texto: `Tu última copia es de hace ${dias} días (el ${fecha} ${hora})`, viejo: false };
  return { texto: `Tu última copia es de hace ${dias} días (el ${fecha} ${hora}): conviene hacer una ahora`, viejo: true };
}

/** Las copias que se pueden guardar: lo mismo que acepta el backend. */
export const MIN_COPIAS = 1;
export const MAX_COPIAS = 365;

/** Lo que se escribió en "cuántas copias guardar" como número entero entre 1 y 365; `null` si no sirve. */
export function copiasAGuardar(texto: string): number | null {
  const t = texto.trim();
  if (!/^\d{1,4}$/.test(t)) return null;
  const n = Number(t);
  return n >= MIN_COPIAS && n <= MAX_COPIAS ? n : null;
}

/** Con una copia por hora, qué tanto hacia atrás llegan las copias guardadas: "las últimas 5 horas". */
export function alcanceHorario(copias: number): string {
  return copias === 1 ? "la última hora" : `las últimas ${copias} horas`;
}
