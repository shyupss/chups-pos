// Qué valores por defecto recibe una instalación nueva, sin React ni localStorage: se prueba con
// `npm test`. Solo usa tipos, así que Node lo corre tal cual.

export interface EntradaDefectos {
  /** La base está vacía y el asistente de primer arranque sigue pendiente. */
  instalacionNueva: boolean;
  /** Ya hay un tema guardado (alguien lo eligió, o una versión anterior lo dejó). */
  hayTema: boolean;
  /** Ya hay un modo de barra lateral guardado. */
  hayBarra: boolean;
}

export interface DefectosNuevos {
  tema?: "oscuro";
  barra?: "expandida";
}

/**
 * Una instalación nueva arranca en tema Oscuro y con la barra Expandida. Un equipo que ya venía
 * usando la app no cambia, aunque nunca haya elegido nada (sin nada guardado ve "Sistema" y
 * "Automática"): por eso el valor de respaldo de la lectura sigue siendo el de antes y esto solo
 * actúa cuando la instalación es nueva. Lo que alguien ya eligió manda siempre.
 */
export function defectosParaInstalacionNueva(e: EntradaDefectos): DefectosNuevos {
  if (!e.instalacionNueva) return {};
  return {
    ...(e.hayTema ? {} : { tema: "oscuro" as const }),
    ...(e.hayBarra ? {} : { barra: "expandida" as const }),
  };
}
