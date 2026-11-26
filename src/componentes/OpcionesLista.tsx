import type { Opcion } from "./Opciones";

export interface OpcionConDetalle<T extends string> extends Opcion<T> {
  /** Una frase que dice qué pasa al elegirla. */
  detalle: string;
}

/**
 * Radios nativos en una lista vertical, uno por fila y con una frase de explicación: para elecciones con
 * textos largos, donde una barra de segmentos se cortaría en dos filas al agrandar la interfaz.
 * Las flechas cambian la opción, como en Opciones.
 */
export default function OpcionesLista<T extends string>({
  nombre,
  etiqueta,
  opciones,
  valor,
  onCambiar,
}: {
  nombre: string;
  etiqueta: string;
  opciones: OpcionConDetalle<T>[];
  valor: T;
  onCambiar: (valor: T) => void;
}) {
  return (
    <div role="radiogroup" aria-label={etiqueta} className="flex flex-col gap-2">
      {opciones.map((o) => (
        <label
          key={o.id}
          className={`flex cursor-pointer items-start gap-3 rounded-lg border-2 px-3 py-2 has-focus-visible:outline-2 has-focus-visible:outline-offset-2 has-focus-visible:outline-acento ${
            valor === o.id ? "border-acento bg-seleccion" : "border-linea hover:border-borde-control"
          }`}
        >
          <input
            type="radio"
            name={nombre}
            value={o.id}
            checked={valor === o.id}
            onChange={() => onCambiar(o.id)}
            className="mt-1 size-4 shrink-0 accent-acento"
          />
          <span className="min-w-0">
            <span className={`block ${valor === o.id ? "font-semibold text-acento" : "font-medium"}`}>{o.nombre}</span>
            <span className="block text-sm text-tenue">{o.detalle}</span>
          </span>
        </label>
      ))}
    </div>
  );
}
