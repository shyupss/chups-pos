import { useId, type ReactNode } from "react";
import { useAtajos } from "../atajos";

interface Props {
  titulo: string;
  onCerrar: () => void;
  children: ReactNode;
  /** Clase de ancho de Tailwind; por defecto 24rem. */
  ancho?: string;
}

/**
 * Diálogo pequeño sobre la Caja, con sombra --s3, la más alta de la interfaz. Mientras está
 * abierto, los atajos de sección y de pantalla no corren; Esc lo cierra aunque el foco no esté
 * dentro (lo atiende el despachador de atajos).
 */
export default function Dialogo({ titulo, onCerrar, children, ancho = "w-[24rem]" }: Props) {
  const id = useId();
  useAtajos("dialogo", { Escape: () => onCerrar() });
  return (
    <div className="fixed inset-0 z-20 flex items-center justify-center bg-velo p-4">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={id}
        className={`${ancho} max-h-full max-w-full overflow-y-auto tarjeta p-6 shadow-s3`}
      >
        <h2 id={id} className="text-lg font-semibold">
          {titulo}
        </h2>
        {children}
      </div>
    </div>
  );
}
