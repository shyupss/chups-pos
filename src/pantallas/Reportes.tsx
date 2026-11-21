import Hoy from "./reportes/Hoy";

export type PestanaReportes = "hoy";

const PESTANAS: { id: PestanaReportes; nombre: string }[] = [{ id: "hoy", nombre: "Hoy" }];

interface Props {
  activa: boolean;
  pestana: PestanaReportes;
  onPestana: (p: PestanaReportes) => void;
  onVerBajoMinimo: () => void;
  onVerCliente: (id: number) => void;
  onVerProducto: (p: { id: number; nombre: string }) => void;
}

/** Reportes (F6): cómo va el negocio hoy. F6 siempre abre Hoy. */
export default function Reportes({ activa, pestana, onPestana, onVerBajoMinimo, onVerCliente, onVerProducto }: Props) {
  return (
    <div className="flex h-full flex-col overflow-auto p-6">
      <div role="tablist" aria-label="Reportes" className="mb-4 flex gap-1 border-b border-linea">
        {PESTANAS.map((p) => (
          <button
            key={p.id}
            type="button"
            role="tab"
            aria-selected={pestana === p.id}
            onClick={() => onPestana(p.id)}
            className={`-mb-px h-10 border-b-2 px-4 font-medium ${
              pestana === p.id ? "border-acento text-acento" : "border-transparent text-tenue hover:text-texto"
            }`}
          >
            {p.nombre}
          </button>
        ))}
      </div>
      {pestana === "hoy" && (
        <Hoy activa={activa} onVerBajoMinimo={onVerBajoMinimo} onVerCliente={onVerCliente} onVerProducto={onVerProducto} />
      )}
    </div>
  );
}
