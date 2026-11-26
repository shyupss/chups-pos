// Reglas del despachador de atajos, sin React ni DOM: se prueban con `npm test`.
// El despachador (src/atajos.ts) solo traduce el evento y llama a lo que esto decide.

/**
 * Dónde vale un atajo:
 * - global: las secciones (F1–F6, F9);
 * - pantalla: la sección a la vista (F7 y F8 en la Caja, Ctrl+E en Reportes…);
 * - dialogo: un diálogo abierto. El de más arriba gana y bloquea los demás ámbitos.
 */
export type Ambito = "global" | "pantalla" | "dialogo";

/** Clave especial: cualquier carácter suelto (el "foco pegajoso" de la Caja y de Ventas). */
export const IMPRIMIBLE = "imprimible";

/** Lo mínimo de un evento de teclado que hace falta para decidir. */
export interface Tecla {
  key: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

export interface Registro {
  ambito: Ambito;
  /** Orden de aparición: el mayor está más arriba (se abrió o se dibujó después). */
  orden: number;
  /** Las combinaciones que atiende ("F12", "Ctrl+P", "Alt+1", "Escape", "1", IMPRIMIBLE…). */
  teclas: readonly string[];
}

const ES_F = /^F\d{1,2}$/;

/**
 * La combinación como texto: "F1", "Ctrl+P", "Alt+1", "Ctrl+Shift+F5", "Escape", "1".
 * Las letras van en mayúscula. En un carácter suelto no se anota Shift (ya cambia el carácter:
 * "+" en un teclado latinoamericano se escribe con Shift).
 */
export function combinacion(t: Tecla): string {
  let key = t.key;
  if (key === " ") key = "Space";
  const suelto = key.length === 1;
  if (suelto) key = key.toUpperCase();
  const partes: string[] = [];
  if (t.ctrlKey) partes.push("Ctrl");
  if (t.altKey) partes.push("Alt");
  if (t.shiftKey && !suelto) partes.push("Shift");
  if (t.metaKey) partes.push("Meta");
  partes.push(key);
  return partes.join("+");
}

/** Un carácter suelto, sin Ctrl, Alt ni Meta: una letra, un dígito o un signo. */
export function esCaracterSuelto(combo: string): boolean {
  return combo.length === 1;
}

/** Las F y lo que va con Ctrl, Alt o Meta: funcionan aunque el foco esté en un campo. */
export function esDeCaptura(combo: string): boolean {
  return ES_F.test(combo) || /^(Ctrl|Alt|Meta)\+/.test(combo) || /^Shift\+F\d{1,2}$/.test(combo);
}

// Teclas del navegador que nunca deben hacer lo suyo en la app (además de desactivarlas en Rust):
// recargar, buscar, imprimir la página, guardarla, ver el código, descargas, historial, ventana
// nueva, zoom, atrás y adelante.
const DEL_NAVEGADOR = new Set([
  "Ctrl+F",
  "Ctrl+G",
  "Ctrl+H",
  "Ctrl+J",
  "Ctrl+N",
  "Ctrl+O",
  "Ctrl+P",
  "Ctrl+R",
  "Ctrl+S",
  "Ctrl+U",
  "Ctrl+W",
  "Ctrl+Shift+R",
  "Ctrl+Shift+N",
  "Ctrl+Shift+P",
  "Ctrl+Shift+S",
  "Ctrl++",
  "Ctrl+=",
  "Ctrl+-",
  "Ctrl+0",
  "Alt+ArrowLeft",
  "Alt+ArrowRight",
  "Alt+Home",
  "BrowserBack",
  "BrowserForward",
  "BrowserRefresh",
  "BrowserSearch",
  "BrowserHome",
]);

/** Si el navegador haría algo con esta tecla: se le pone preventDefault aunque nadie la atienda. */
export function esTeclaDelNavegador(combo: string): boolean {
  // Todas las F, con o sin Shift/Ctrl (F5, Shift+F5, Ctrl+F5, F3, F7, F12…). Alt+F4 no: cierra la ventana.
  if (/^(Ctrl\+)?(Shift\+)?F\d{1,2}$/.test(combo)) return true;
  return DEL_NAVEGADOR.has(combo);
}

/**
 * Quién atiende la tecla, o null. `enCampo`: el foco está en un campo editable.
 * - Un carácter suelto no se toma en un campo: es texto que se está escribiendo.
 * - Con un diálogo abierto solo cuenta el de más arriba: ni la pantalla ni las secciones.
 * - Sin diálogo: la pantalla (el registro más reciente primero) y después lo global.
 */
export function resolver<R extends Registro>(
  registros: readonly R[],
  combo: string,
  enCampo: boolean,
): { registro: R; clave: string } | null {
  const suelto = esCaracterSuelto(combo);
  if (suelto && enCampo) return null;
  const atiende = (r: R) => {
    if (r.teclas.includes(combo)) return combo;
    if (suelto && r.teclas.includes(IMPRIMIBLE)) return IMPRIMIBLE;
    return null;
  };

  const dialogos = registros.filter((r) => r.ambito === "dialogo");
  if (dialogos.length > 0) {
    const arriba = dialogos.reduce((a, b) => (b.orden > a.orden ? b : a));
    const clave = atiende(arriba);
    return clave ? { registro: arriba, clave } : null;
  }

  const porOrden = (a: R, b: R) => b.orden - a.orden;
  for (const ambito of ["pantalla", "global"] as const) {
    for (const r of registros.filter((x) => x.ambito === ambito).sort(porOrden)) {
      const clave = atiende(r);
      if (clave) return { registro: r, clave };
    }
  }
  return null;
}
