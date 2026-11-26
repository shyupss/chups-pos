// Preferencias de la interfaz. Por ahora en localStorage; en otra fase pasan a la tabla config.

export function leerPreferencia<T extends string>(clave: string, validos: readonly T[], defecto: T): T {
  try {
    const v = localStorage.getItem(clave);
    if (v !== null && (validos as readonly string[]).includes(v)) return v as T;
  } catch {
    // Sin acceso a localStorage: se usa el valor por defecto.
  }
  return defecto;
}

/** Si hay un valor válido guardado (alguien lo eligió): distingue "eligió el de siempre" de "no eligió". */
export function hayPreferencia(clave: string, validos: readonly string[]): boolean {
  try {
    const v = localStorage.getItem(clave);
    return v !== null && validos.includes(v);
  } catch {
    return false;
  }
}

export function guardarPreferencia(clave: string, valor: string) {
  try {
    localStorage.setItem(clave, valor);
  } catch {
    // Si no se puede guardar, el cambio vale igual para esta sesión.
  }
}
