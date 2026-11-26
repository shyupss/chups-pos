import { useSyncExternalStore } from "react";
import { guardarPreferencia, hayPreferencia, leerPreferencia } from "../preferencias";

export type Tema = "claro" | "oscuro" | "sistema";

const CLAVE = "chups-pos.tema";
const TEMAS: readonly Tema[] = ["claro", "oscuro", "sistema"];
const consultaOscuro = window.matchMedia("(prefers-color-scheme: dark)");
const oyentes = new Set<() => void>();
// Sin nada guardado se ve "sistema", como siempre: el oscuro de las instalaciones nuevas lo fija
// App.tsx al guardarlo (src/preferencias-nucleo.ts), para no cambiar equipos que ya usaban la app.
let tema = leerPreferencia<Tema>(CLAVE, TEMAS, "sistema");

/** Si alguien ya eligió un tema (o quedó guardado). */
export function hayTemaGuardado() {
  return hayPreferencia(CLAVE, TEMAS);
}

function aplicar() {
  const oscuro = tema === "oscuro" || (tema === "sistema" && consultaOscuro.matches);
  document.documentElement.dataset.theme = oscuro ? "dark" : "light";
}

export function cambiarTema(nuevo: Tema) {
  tema = nuevo;
  guardarPreferencia(CLAVE, nuevo);
  aplicar();
  oyentes.forEach((o) => o());
}

function suscribir(oyente: () => void) {
  oyentes.add(oyente);
  return () => oyentes.delete(oyente);
}

/** Se llama en main.tsx antes del primer render, para no mostrar un destello del otro tema. */
export function iniciarTema() {
  aplicar();
  // En "sistema", sigue los cambios de Windows sin reiniciar la app.
  consultaOscuro.addEventListener("change", aplicar);
}

export function useTema() {
  return [useSyncExternalStore(suscribir, () => tema), cambiarTema] as const;
}
