import { Info } from "lucide-react";
import { useId } from "react";

/**
 * Un ícono (i) con una explicación corta: aparece al pasar el mouse o al llegar con el teclado, y los lectores de
 * pantalla la leen como descripción del botón. Para decir en una frase qué mide una tarjeta.
 */
export default function InfoAyuda({ texto, etiqueta }: { texto: string; etiqueta: string }) {
  const id = useId();
  return (
    <span className="group relative inline-flex shrink-0">
      <button
        type="button"
        aria-label={`¿Qué es «${etiqueta}»?`}
        aria-describedby={id}
        className="inline-flex size-6 items-center justify-center rounded-full text-tenue hover:bg-suave hover:text-texto"
      >
        <Info size={16} aria-hidden />
      </button>
      <span
        id={id}
        role="tooltip"
        className="pointer-events-none invisible absolute top-full right-0 z-30 mt-1 w-64 rounded-lg border border-linea bg-superficie p-3 text-left text-sm font-normal text-texto shadow-s2 group-focus-within:visible group-hover:visible"
      >
        {texto}
      </span>
    </span>
  );
}
