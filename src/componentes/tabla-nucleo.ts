// Reglas de la tabla común (src/componentes/Tabla.tsx), sin React: se prueban con `npm test`.

export type Direccion = "asc" | "desc";

export interface Orden {
  columna: string;
  direccion: Direccion;
}

/** Al hacer clic en un encabezado: ascendente, luego descendente y luego sin orden (el original). */
export function siguienteOrden(actual: Orden | null, columna: string): Orden | null {
  if (actual?.columna !== columna) return { columna, direccion: "asc" };
  return actual.direccion === "asc" ? { columna, direccion: "desc" } : null;
}

type Valor = string | number | null | undefined;

/**
 * Ordena las filas ya cargadas por el valor de una columna. Estable (a igual valor se mantiene el
 * orden de antes), los números van como números, el texto sin distinguir mayúsculas ni tildes, y lo
 * que no tiene valor va siempre al final, ascendente o descendente.
 */
export function ordenarFilas<T>(filas: readonly T[], valor: (fila: T) => Valor, direccion: Direccion): T[] {
  const signo = direccion === "asc" ? 1 : -1;
  const vacio = (v: Valor) => v === null || v === undefined || v === "";
  return filas
    .map((fila, i) => ({ fila, i, v: valor(fila) }))
    .sort((a, b) => {
      if (vacio(a.v) || vacio(b.v)) return vacio(a.v) === vacio(b.v) ? a.i - b.i : vacio(a.v) ? 1 : -1;
      const c =
        typeof a.v === "number" && typeof b.v === "number"
          ? a.v - b.v
          : String(a.v).localeCompare(String(b.v), "es", { sensitivity: "base", numeric: true });
      return c === 0 ? a.i - b.i : c * signo;
    })
    .map((x) => x.fila);
}

export interface ColumnaVisible {
  /** Se oculta cuando la tabla mide menos que esto (px CSS). Sin valor, siempre se ve. */
  ocultarBajo?: number;
}

/** Las columnas que caben en una tabla de `ancho` px. Con 0 (todavía sin medir) se muestran todas. */
export function columnasVisibles<C extends ColumnaVisible>(columnas: readonly C[], ancho: number): C[] {
  if (ancho <= 0) return [...columnas];
  return columnas.filter((c) => c.ocultarBajo === undefined || ancho >= c.ocultarBajo);
}

/**
 * La plantilla del grid de cada fila. Con `conFlecha` sobra una última pista `auto`: si ninguna columna es
 * flexible (`1fr`), absorbe el espacio libre, así las columnas no se separan en pantallas anchas y la flecha "›"
 * queda al borde derecho; si hay una columna flexible, esa se lleva el espacio y la flecha se queda en su mínimo.
 */
export function plantillaGrid(anchos: readonly string[], conFlecha: boolean): string {
  return [...anchos, conFlecha ? "minmax(1.5rem,auto)" : ""].filter(Boolean).join(" ");
}

/** "1.234 productos", "1 producto"; con más por cargar, "100 productos (baja para ver más)". */
export function textoConteo(n: number, singular: string, plural: string, hayMas = false): string {
  const cantidad = n.toLocaleString("es-CL");
  const base = `${cantidad} ${n === 1 ? singular : plural}`;
  return hayMas ? `${base} (baja para ver más)` : base;
}
