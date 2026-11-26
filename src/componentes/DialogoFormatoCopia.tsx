import { useState } from "react";
import Boton from "./Boton";
import Dialogo from "./Dialogo";

export type FormatoCopia = "completa" | "excel";

/**
 * Antes de hacer una copia o llevarla a otro computador: ¿en qué formato? La copia completa es la única que se
 * puede importar para recuperar todo; la planilla Excel sirve para ver o guardar los datos.
 */
export default function DialogoFormatoCopia({
  titulo,
  boton,
  ocupado,
  onElegir,
  onCerrar,
}: {
  titulo: string;
  boton: string;
  ocupado?: boolean;
  onElegir: (formato: FormatoCopia) => void;
  onCerrar: () => void;
}) {
  const [formato, setFormato] = useState<FormatoCopia>("completa");
  const opciones: { id: FormatoCopia; nombre: string; detalle: string }[] = [
    { id: "completa", nombre: "Copia completa (se puede importar)", detalle: "Todo tu negocio en un solo archivo. Es la que sirve para recuperar los datos o pasarlos a otro computador." },
    {
      id: "excel",
      nombre: "Planilla Excel con todos los datos",
      detalle: "Para ver o guardar tus datos; no se puede importar como copia. Si algo se pierde, no sirve para recuperarlo.",
    },
  ];
  return (
    <Dialogo titulo={titulo} onCerrar={onCerrar} ancho="w-[34rem]">
      <p className="mt-2 text-tenue">¿En qué formato la quieres?</p>
      <div role="radiogroup" aria-label="Formato de la copia" className="mt-3 flex flex-col gap-2">
        {opciones.map((o) => (
          <label
            key={o.id}
            className={`flex cursor-pointer items-start gap-3 rounded-lg border-2 p-3 has-focus-visible:outline-2 has-focus-visible:outline-offset-2 has-focus-visible:outline-acento ${
              formato === o.id ? "border-acento bg-seleccion" : "border-linea hover:border-borde-control"
            }`}
          >
            <input type="radio" name="formato-copia" checked={formato === o.id} onChange={() => setFormato(o.id)} className="mt-1 size-4 accent-acento" />
            <span>
              <span className="block font-medium">{o.nombre}</span>
              <span className="block text-sm text-tenue">{o.detalle}</span>
            </span>
          </label>
        ))}
      </div>
      <div className="mt-5 flex gap-2">
        <Boton variante="terciario" atajo="Esc" onClick={onCerrar} className="flex-1">
          Cancelar
        </Boton>
        <Boton variante="primario" onClick={() => onElegir(formato)} disabled={ocupado} className="flex-[2]">
          {boton}
        </Boton>
      </div>
    </Dialogo>
  );
}
