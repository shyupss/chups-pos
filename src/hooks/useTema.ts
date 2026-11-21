import { useSyncExternalStore } from "react";
import { guardarPreferencia, leerPreferencia } from "../preferencias";

export type Tema = "claro" | "oscuro" | "sistema";

const CLAVE = "chups-pos.tema";
const consultaOscuro = window.matchMedia("(prefers-color-scheme: dark)");
const oyentes = new Set<() => void>();
let tema = leerPreferencia<Tema>(CLAVE, ["claro", "oscuro", "sistema"], "sistema");

function aplicar() {
  const oscuro = tema === "oscuro" || (tema === "sistema" && consultaOscuro.matches);
  document.documentElement.dataset.theme = oscuro ? "dark" : "light";
}

function cambiarTema(nuevo: Tema) {
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
