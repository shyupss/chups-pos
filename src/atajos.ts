// El único despachador de atajos de la app. Hay un solo listener de keydown en window, instalado
// en main.tsx, y los componentes registran sus atajos con useAtajos(ámbito, mapa) en vez de
// poner su propio listener. Las reglas (quién gana, qué pasa en un campo) están en atajos-nucleo.ts.
//
// Dos fases:
// - Captura: las F y lo que lleva Ctrl, Alt o Meta. Se atienden antes que nadie, aunque el foco
//   esté en un campo o un componente detenga el evento: así nunca se pierde un atajo.
// - Burbuja: Esc, Enter y los caracteres sueltos. Llegan después de los onKeyDown de los campos;
//   si uno ya la atendió (preventDefault o stopPropagation), el atajo no corre. Así Enter en el
//   buscador de clientes o Esc en una celda en edición siguen haciendo lo suyo.

import { useEffect, useRef, useState } from "react";
import { combinacion, esDeCaptura, esTeclaDelNavegador, resolver, type Ambito, type Registro } from "./atajos-nucleo";

export { IMPRIMIBLE } from "./atajos-nucleo";

/**
 * Lo que hace la tecla. Si devuelve false, no la consumió: no se le pone preventDefault y la tecla
 * sigue su camino (por ejemplo, el foco pegajoso de la Caja enfoca el campo y deja que el carácter
 * se escriba ahí).
 */
export type Manejador = (e: KeyboardEvent) => void | boolean;
export type MapaAtajos = Partial<Record<string, Manejador>>;

interface RegistroVivo extends Registro {
  mapa: { current: MapaAtajos };
}

const registros = new Set<RegistroVivo>();
let contador = 0;

/** El foco está donde se escribe texto: ahí las letras sueltas son texto, no atajos. */
function enCampoEditable(destino: EventTarget | null): boolean {
  if (!(destino instanceof HTMLElement)) return false;
  if (destino.isContentEditable) return true;
  if (destino instanceof HTMLTextAreaElement || destino instanceof HTMLSelectElement) return true;
  if (destino instanceof HTMLInputElement) {
    return !["checkbox", "radio", "button", "submit", "reset", "range", "color", "file"].includes(destino.type);
  }
  return false;
}

function despachar(e: KeyboardEvent, captura: boolean) {
  if (e.isComposing || e.key === "Process" || e.key === "Dead" || e.key === "Unidentified") return;
  if (["Control", "Alt", "Shift", "Meta", "AltGraph"].includes(e.key)) return;
  const combo = combinacion(e);
  if (esDeCaptura(combo) !== captura) return;
  if (!captura && e.defaultPrevented) return;
  const lista = [...registros].map((r) => ({ ...r, teclas: Object.keys(r.mapa.current) }));
  const elegido = resolver(lista, combo, enCampoEditable(e.target));
  const manejador = elegido ? elegido.registro.mapa.current[elegido.clave] : undefined;
  const consumida = manejador ? manejador(e) !== false : false;
  if (consumida || (captura && esTeclaDelNavegador(combo))) e.preventDefault();
}

let instalado = false;

/** Se llama una vez al arrancar (main.tsx). */
export function instalarAtajos() {
  if (instalado) return;
  instalado = true;
  window.addEventListener("keydown", (e) => despachar(e, true), { capture: true });
  window.addEventListener("keydown", (e) => despachar(e, false));
}

/**
 * Registra atajos mientras el componente está montado y `activo` es true. El mapa se puede
 * escribir en línea: se lee en cada tecla, sin volver a registrarse en cada render.
 *
 * El orden se toma al primer render, que va de padre a hijo y en el orden en que se abren las
 * cosas: un diálogo que se abre después queda arriba, y el detalle de una venta gana a la lista.
 */
export function useAtajos(ambito: Ambito, mapa: MapaAtajos, activo = true) {
  const [orden] = useState(() => ++contador);
  const ref = useRef(mapa);
  ref.current = mapa;
  useEffect(() => {
    if (!activo) return;
    const registro: RegistroVivo = { ambito, orden, teclas: [], mapa: ref };
    registros.add(registro);
    return () => {
      registros.delete(registro);
    };
  }, [ambito, orden, activo]);
}
