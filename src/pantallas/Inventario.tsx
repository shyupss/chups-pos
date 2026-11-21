import { useEffect, useState } from "react";
import RegistrarCompra from "../componentes/RegistrarCompra";
import { avisarCambioStock } from "../eventos";
import Compras from "./inventario/Compras";
import Movimientos from "./inventario/Movimientos";
import Proveedores from "./inventario/Proveedores";
import Stock from "./inventario/Stock";

export type PestanaInventario = "stock" | "movimientos" | "compras" | "proveedores";

const PESTANAS: { id: PestanaInventario; nombre: string }[] = [
  { id: "stock", nombre: "Stock" },
  { id: "movimientos", nombre: "Movimientos" },
  { id: "compras", nombre: "Compras" },
  { id: "proveedores", nombre: "Proveedores" },
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

/** Inventario (F3): stock con su valor, movimientos, compras y proveedores. Ctrl+N: nueva compra. */
export default function Inventario({ activa, pestana, onPestana, producto, onProducto, onIrAVenta }: Props) {
  const [nuevaCompra, setNuevaCompra] = useState(false);
  const [compraElegida, setCompraElegida] = useState<number | null>(null);
  const [version, setVersion] = useState(0);

  useEffect(() => {
    if (!activa) return;
    const alPresionar = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key.toLowerCase() === "n" && !document.querySelector('[role="dialog"]')) {
        e.preventDefault();
        setNuevaCompra(true);
      }
    };
    window.addEventListener("keydown", alPresionar);
    return () => window.removeEventListener("keydown", alPresionar);
  }, [activa]);

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
        <Movimientos
          activa={activa}
          producto={producto}
          onProducto={onProducto}
          onIrAVenta={onIrAVenta}
          onIrACompra={(id) => {
            setCompraElegida(id);
            onPestana("compras");
          }}
        />
      )}
      {pestana === "compras" && (
        <Compras
          activa={activa}
          elegida={compraElegida}
          onElegir={setCompraElegida}
          onNueva={() => setNuevaCompra(true)}
          version={version}
        />
      )}
      {pestana === "proveedores" && <Proveedores activa={activa} />}

      {nuevaCompra && activa && (
        <RegistrarCompra
          onCerrar={() => setNuevaCompra(false)}
          onRegistrada={(d) => {
            setNuevaCompra(false);
            setCompraElegida(d.compra.id);
            setVersion((v) => v + 1);
            onPestana("compras");
            avisarCambioStock();
          }}
        />
      )}
    </div>
  );
}
