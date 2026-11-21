import { useId } from "react";

/**
 * Ajuste de sí/no (activado, pide PIN…). Es un checkbox real con role="switch": se alcanza con Tab
 * y Espacio lo cambia. `etiqueta` es el nombre accesible cuando no hay un texto visible al lado.
 */
export default function Interruptor({
  etiqueta,
  marcado,
  onCambiar,
  desactivado,
}: {
  etiqueta: string;
  marcado: boolean;
  onCambiar: (valor: boolean) => void;
  desactivado?: boolean;
}) {
  const id = useId();
  return (
    <input
      id={id}
      type="checkbox"
      role="switch"
      aria-label={etiqueta}
      className="interruptor"
      checked={marcado}
      disabled={desactivado}
      onChange={(e) => onCambiar(e.target.checked)}
    />
  );
}
