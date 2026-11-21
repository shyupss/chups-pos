import type { ReactNode } from "react";

/**
 * Las dos zonas de una sección de Configuración: cada una es una columna de tarjetas. Con menos de
 * 1100 px de ancho de contenido pasan a una sola columna (el contenedor es el área de la sección).
 */
export function Columnas({ izquierda, derecha }: { izquierda: ReactNode; derecha?: ReactNode }) {
  return (
    <div className="grid grid-cols-1 items-start gap-5 @min-[1100px]:grid-cols-[minmax(0,1.15fr)_minmax(0,1fr)]">
      <div className="flex min-w-0 flex-col gap-5">{izquierda}</div>
      {derecha && <div className="flex min-w-0 flex-col gap-5">{derecha}</div>}
    </div>
  );
}

/** Tarjeta de Configuración: título, una línea de ayuda y el cuerpo con 16 px entre ajustes. */
export function Tarjeta({
  titulo,
  ayuda,
  children,
}: {
  titulo: string;
  ayuda?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="tarjeta p-5">
      <h2 className="text-[15px] font-semibold">{titulo}</h2>
      {ayuda && <p className="mt-0.5 text-[13px] text-tenue">{ayuda}</p>}
      <div className="mt-4 flex flex-col gap-4">{children}</div>
    </section>
  );
}

/**
 * Un ajuste: su nombre (y una ayuda de dos líneas como mucho) a la izquierda y el control a la
 * derecha, que nunca se encoge. Entre ajustes hay una línea fina.
 */
export function Ajuste({ etiqueta, ayuda, children }: { etiqueta: string; ayuda?: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 border-t border-linea pt-4 first:border-t-0 first:pt-0">
      <div className="min-w-0">
        <div className="text-sm font-medium">{etiqueta}</div>
        {ayuda && <div className="text-[12.5px] text-tenue">{ayuda}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}
