import { useState } from "react";
import type { Producto } from "../api";
import { cantidad, cantidadEditable, MAX_KG_MIL, parseKg, pesos, subtotalLinea } from "../formato";
import Dialogo from "./Dialogo";

interface Props {
  producto: Pick<Producto, "nombre" | "precioVenta">;
  /** Peso actual de la línea, para "Cambiar peso". */
  inicialMil?: number;
  /** Tope en gramos; por defecto 100 kg. En una devolución, lo que queda por devolver. */
  maximoMil?: number;
  /** Mínimo en gramos; por defecto 1. Un conteo de inventario acepta 0. */
  minimoMil?: number;
  /** Texto del botón (por defecto "Agregar" o "Cambiar peso"). */
  accion?: string;
  onConfirmar: (cantidadMil: number) => void;
  onCancelar: () => void;
}

export default function DialogoPeso({
  producto,
  inicialMil,
  maximoMil,
  minimoMil = 1,
  accion,
  onConfirmar,
  onCancelar,
}: Props) {
  const [texto, setTexto] = useState(inicialMil ? cantidadEditable(inicialMil, "kg") : "");
  const maximo = maximoMil ?? MAX_KG_MIL;
  const mil = parseKg(texto);
  const valido = mil !== null && mil >= minimoMil && mil <= maximo;
  const error =
    texto.trim() === "" || valido
      ? null
      : mil === null
        ? "Escribe el peso en kg (0,532) o en gramos (532 g)"
        : maximoMil !== undefined
          ? `El peso debe estar entre ${minimoMil} g y ${cantidad(maximo, "kg")}`
          : `El peso debe estar entre ${minimoMil} g y 100 kg`;

  return (
    <Dialogo titulo={`Peso de ${producto.nombre}`} onCerrar={onCancelar}>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (valido) onConfirmar(mil);
        }}
      >
        <input
          autoFocus
          onFocus={(e) => e.currentTarget.select()}
          value={texto}
          onChange={(e) => setTexto(e.target.value)}
          placeholder="0,532 o 532 g"
          aria-label="Peso en kg o en gramos"
          aria-invalid={error !== null}
          className="mt-4 h-13 w-full rounded-lg border border-borde-control bg-superficie px-4 text-2xl tabular-nums focus:border-acento"
          autoComplete="off"
          spellCheck={false}
        />
        <dl className="mt-4 space-y-1 tabular-nums">
          <div className="flex items-baseline justify-between text-tenue">
            <dt>Precio por kg</dt>
            <dd>{pesos(producto.precioVenta)}</dd>
          </div>
          <div className="flex items-baseline justify-between">
            <dt className="text-tenue">Subtotal</dt>
            <dd className="text-2xl font-semibold">
              {valido ? pesos(subtotalLinea(producto.precioVenta, mil)) : "—"}
            </dd>
          </div>
        </dl>
        {error && <p className="mt-3 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}
        <div className="mt-5 flex gap-2">
          <button
            type="button"
            onClick={onCancelar}
            className="h-11 flex-1 rounded-lg text-tenue hover:bg-fondo hover:text-texto"
          >
            Cancelar <span className="text-xs opacity-60">Esc</span>
          </button>
          <button
            type="submit"
            disabled={!valido}
            className="h-11 flex-[2] rounded-lg bg-acento font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
          >
            {accion ?? (inicialMil ? "Cambiar peso" : "Agregar")} <span className="text-xs font-normal opacity-70">Enter</span>
          </button>
        </div>
      </form>
    </Dialogo>
  );
}
