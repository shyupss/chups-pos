import { useId, type ReactNode } from "react";

interface Props {
  titulo: string;
  onCerrar: () => void;
  children: ReactNode;
  /** Clase de ancho de Tailwind; por defecto 24rem. */
  ancho?: string;
}

/** Diálogo pequeño sobre la Caja. Esc lo cierra; sin sombra (solo el cobro la lleva). */
export default function Dialogo({ titulo, onCerrar, children, ancho = "w-[24rem]" }: Props) {
  const id = useId();
  return (
    <div
      className="fixed inset-0 z-20 flex items-center justify-center bg-velo p-4"
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onCerrar();
        }
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={id}
        className={`${ancho} max-h-full max-w-full overflow-y-auto rounded-lg border border-linea bg-superficie p-6`}
      >
        <h2 id={id} className="text-lg font-semibold">
          {titulo}
        </h2>
        {children}
      </div>
    </div>
  );
}
