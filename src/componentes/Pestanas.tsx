/**
 * Las pestañas de una pantalla (Inventario, Reportes): texto de 15 px y, en la activa, una raya de 3 px
 * del color de acción bajo el texto. Cada una es un `<button role="tab">`; las flechas mueven entre ellas.
 */
export default function Pestanas<T extends string>({
  etiqueta,
  pestanas,
  activa,
  onCambiar,
}: {
  etiqueta: string;
  pestanas: readonly { id: T; nombre: string }[];
  activa: T;
  onCambiar: (id: T) => void;
}) {
  return (
    <div
      role="tablist"
      aria-label={etiqueta}
      className="mb-4 flex flex-wrap gap-x-1 border-b border-linea"
      onKeyDown={(e) => {
        if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
        const i = pestanas.findIndex((p) => p.id === activa);
        const siguiente = pestanas[(i + (e.key === "ArrowRight" ? 1 : pestanas.length - 1)) % pestanas.length];
        e.preventDefault();
        onCambiar(siguiente.id);
        // El foco acompaña a la pestaña elegida.
        requestAnimationFrame(() => e.currentTarget.querySelector<HTMLButtonElement>('[aria-selected="true"]')?.focus());
      }}
    >
      {pestanas.map((p) => {
        const elegida = p.id === activa;
        return (
          <button
            key={p.id}
            type="button"
            role="tab"
            aria-selected={elegida}
            tabIndex={elegida ? 0 : -1}
            onClick={() => onCambiar(p.id)}
            className={`-mb-px h-11 rounded-t-lg border-b-[3px] px-4 text-[15px] ${
              elegida
                ? "border-acento font-semibold text-acento"
                : "border-transparent font-medium text-tenue hover:bg-suave hover:text-texto"
            }`}
          >
            {p.nombre}
          </button>
        );
      })}
    </div>
  );
}
