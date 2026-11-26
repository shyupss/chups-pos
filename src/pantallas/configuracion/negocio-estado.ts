// El estado de Configuración > Negocio, sin React: se prueba con `npm test`.
// Solo usa tipos (se borran al ejecutar), así que Node lo corre tal cual.

import type { DatosNegocio, Negocio } from "../../api";

export interface EstadoNegocio {
  /** Lo que está escrito en el formulario (puede no estar guardado). */
  datos: DatosNegocio;
  /** Lo último guardado en la base: contra esto se ve si hay cambios. */
  guardados: DatosNegocio;
  logo: string | null;
}

export const DATOS_VACIOS: DatosNegocio = { nombre: "", rut: "", direccion: "", telefono: "", mensajePie: "" };

/** Los campos del formulario a partir de lo guardado (los opcionales vacíos quedan en ""). */
export function datosDe(n: Negocio): DatosNegocio {
  return {
    nombre: n.nombre,
    rut: n.rut ?? "",
    direccion: n.direccion ?? "",
    telefono: n.telefono ?? "",
    mensajePie: n.mensajePie,
  };
}

/** Carga inicial y "Guardar datos": todo pasa a ser lo guardado. */
export function desdeGuardado(n: Negocio): EstadoNegocio {
  const datos = datosDe(n);
  return { datos, guardados: datos, logo: n.logoPng };
}

/**
 * Elegir o quitar el logo: solo cambia el logo. Lo escrito y no guardado se queda tal cual (antes
 * se reemplazaba todo por lo que había en la base y se perdía).
 */
export function conLogo(estado: EstadoNegocio, n: Negocio): EstadoNegocio {
  return { ...estado, logo: n.logoPng };
}

/** Si lo escrito es distinto de lo guardado. */
export function hayCambios(datos: DatosNegocio, guardados: DatosNegocio): boolean {
  const campos: (keyof DatosNegocio)[] = ["nombre", "rut", "direccion", "telefono", "mensajePie"];
  return campos.some((c) => (datos[c] ?? "") !== (guardados[c] ?? ""));
}
