import type { DatosSeccion } from "../secciones";

/** Marcador de una sección que llega en una fase posterior. Sin lógica. */
export default function Proximamente({ seccion: { nombre, pendiente, Icono } }: { seccion: DatosSeccion }) {
  return (
    <div className="flex h-full items-center justify-center p-6">
      <div className="max-w-md text-center">
        <Icono size={40} strokeWidth={1.5} className="mx-auto text-tenue" aria-hidden />
        <h1 className="mt-4 text-xl font-semibold">{nombre}</h1>
        <p className="mt-2 text-tenue">{pendiente}</p>
      </div>
    </div>
  );
}
