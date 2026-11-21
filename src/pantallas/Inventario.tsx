import Movimientos from "./inventario/Movimientos";
import Stock from "./inventario/Stock";

export type PestanaInventario = "stock" | "movimientos";

const PESTANAS: { id: PestanaInventario; nombre: string }[] = [
  { id: "stock", nombre: "Stock" },
  { id: "movimientos", nombre: "Movimientos" },
];

interface Props {
  activa: boolean;
  pestana: PestanaInventario;
  onPestana: (p: PestanaInventario) => void;
  /** Producto filtrado en Movimientos (lo fija "Ver movimientos" desde Stock o Productos). */
  producto: { id: number; nombre: string } | null;
  onProducto: (p: { id: number; nombre: string } | null) => void;
  onIrAVenta: (folio: number) => void;
}

/** Inventario (F3): stock con su valor, movimientos, compras y proveedores. */
export default function Inventario({ activa, pestana, onPestana, producto, onProducto, onIrAVenta }: Props) {
  return (
    <div className="flex h-full flex-col p-6">
      <div role="tablist" aria-label="Inventario" className="mb-4 flex gap-1 border-b border-linea">
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
      {pestana === "stock" && (
        <Stock
          activa={activa}
          onVerMovimientos={(p) => {
            onProducto(p);
            onPestana("movimientos");
          }}
        />
      )}
      {pestana === "movimientos" && (
        <Movimientos activa={activa} producto={producto} onProducto={onProducto} onIrAVenta={onIrAVenta} />
      )}
    </div>
  );
}
