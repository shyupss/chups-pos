// Salir con cambios sin guardar. Una pantalla con un formulario a medio editar registra una
// guardia; quien cambia de sección (App, Configuración, el asistente) pregunta antes con
// pedirSalida. Hay una sola guardia a la vez: solo un formulario está a la vista.

import { useEffect, useRef } from "react";

/** Recibe cómo seguir y se encarga de preguntar; llama a `continuar` solo si se puede salir. */
type Preguntar = (continuar: () => void) => void;

let guardia: { current: Preguntar } | null = null;

/** Sale ya si no hay cambios pendientes; si los hay, la guardia pregunta y decide. */
export function pedirSalida(continuar: () => void) {
  if (guardia) guardia.current(continuar);
  else continuar();
}

/** Registra la guardia mientras `pendiente` es true (hay cambios sin guardar). */
export function useGuardiaSalida(pendiente: boolean, preguntar: Preguntar) {
  const ref = useRef(preguntar);
  ref.current = preguntar;
  useEffect(() => {
    if (!pendiente) return;
    const propia = ref;
    guardia = propia;
    return () => {
      if (guardia === propia) guardia = null;
    };
  }, [pendiente]);
}
