import type { LucideIcon } from "lucide-react";

export interface Opcion<T extends string> {
  id: T;
  nombre: string;
  Icono?: LucideIcon;
}

/**
 * Grupo de radios nativos con aspecto de segmentos: las flechas cambian la opción. Lo elegido va en
 * el estilo de selección (fondo --seleccion, texto --acento). Para ajustes de sí/no se usa Interruptor.
 */
export default function Opciones<T extends string>({
  nombre,
  etiqueta,
  opciones,
  valor,
  onCambiar,
}: {
  nombre: string;
  etiqueta: string;
  opciones: Opcion<T>[];
  valor: T;
  onCambiar: (valor: T) => void;
}) {
  return (
    <div role="radiogroup" aria-label={etiqueta} className="inline-flex rounded-lg border border-borde-control bg-suave p-0.5">
      {opciones.map(({ id, nombre: texto, Icono }) => (
        <label
          key={id}
          className="relative flex h-8 cursor-pointer items-center gap-2 rounded-md px-3 text-tenue hover:text-texto has-checked:bg-seleccion has-checked:font-semibold has-checked:text-acento has-checked:shadow-s1 has-focus-visible:outline-2 has-focus-visible:outline-offset-2 has-focus-visible:outline-acento"
        >
          <input
            type="radio"
            name={nombre}
            value={id}
            checked={valor === id}
            onChange={() => onCambiar(id)}
            className="sr-only"
          />
          {Icono && <Icono size={16} aria-hidden />}
          {texto}
        </label>
      ))}
    </div>
  );
}
