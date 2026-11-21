import type { ReactNode } from "react";

/** Sección de Configuración. Con `fase`, es un marcador de algo que llega más adelante. */
export function Bloque({ titulo, fase, children }: { titulo: string; fase?: number; children: ReactNode }) {
  return (
    <section className="rounded-lg border border-linea bg-superficie p-5">
      <h2 className="font-semibold">{titulo}</h2>
      {fase === undefined ? (
        <div className="mt-3">{children}</div>
      ) : (
        <p className="mt-1 text-tenue">
          {children} Llega en la fase {fase}.
        </p>
      )}
    </section>
  );
}

/** Ajuste con su nombre y ayuda a la izquierda y el control a la derecha. */
export function Fila({ etiqueta, ayuda, children }: { etiqueta: string; ayuda: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6">
      <div className="min-w-0">
        <div className="font-medium">{etiqueta}</div>
        <div className="text-xs text-tenue">{ayuda}</div>
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}
