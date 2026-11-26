import { useState } from "react";
import { useAtajos } from "../atajos";
import Pestanas from "../componentes/Pestanas";
import RegistrarCompra from "../componentes/RegistrarCompra";
import type { FiltroEstadoStock } from "../api";
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
  /** Filtro con que se abre Stock (desde Reportes > Hoy, "bajo el mínimo"). */
  estadoStock?: FiltroEstadoStock | null;
}

/** Inventario (F3): stock con su valor, movimientos, compras y proveedores. Ctrl+N: nueva compra. */
export default function Inventario({ activa, pestana, onPestana, producto, onProducto, onIrAVenta, estadoStock }: Props) {
  const [nuevaCompra, setNuevaCompra] = useState(false);
  const [compraElegida, setCompraElegida] = useState<number | null>(null);
  const [version, setVersion] = useState(0);

  useAtajos("pantalla", { "Ctrl+N": () => setNuevaCompra(true) }, activa);

  return (
    <div className="flex h-full flex-col p-6">
      <Pestanas etiqueta="Inventario" pestanas={PESTANAS} activa={pestana} onCambiar={onPestana} />
      {pestana === "stock" && (
        <Stock
          activa={activa}
          estadoInicial={estadoStock ?? undefined}
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
