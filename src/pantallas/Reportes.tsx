import { useEffect, useState } from "react";
import { api, type Categoria } from "../api";
import Pestanas from "../componentes/Pestanas";
import Hoy from "./reportes/Hoy";
import { ReporteFiados, ReporteGanancia, ReporteProductos, ReporteTurnos, ReporteVentas } from "./reportes/PorRango";
import { rangoDe, SelectorRango, type Rango } from "./reportes/Rango";

export type PestanaReportes = "hoy" | "ventas" | "productos" | "ganancia" | "turnos" | "fiados";

const PESTANAS: { id: PestanaReportes; nombre: string }[] = [
  { id: "hoy", nombre: "Hoy" },
  { id: "ventas", nombre: "Ventas" },
  { id: "productos", nombre: "Productos" },
  { id: "ganancia", nombre: "Ganancia" },
  { id: "turnos", nombre: "Turnos" },
  { id: "fiados", nombre: "Fiados" },
];

interface Props {
  activa: boolean;
  pestana: PestanaReportes;
  onPestana: (p: PestanaReportes) => void;
  onVerBajoMinimo: () => void;
  onVerCliente: (id: number) => void;
  onVerProducto: (p: { id: number; nombre: string }) => void;
}

/** Reportes (F6): Hoy y los reportes por rango. F6 siempre abre Hoy; en un reporte, Ctrl+E exporta a Excel. */
export default function Reportes({ activa, pestana, onPestana, onVerBajoMinimo, onVerCliente, onVerProducto }: Props) {
  // El rango se comparte entre pestañas: cambiar de Ventas a Ganancia mira el mismo período.
  const [rango, setRango] = useState<Rango>(() => rangoDe("mes"));
  const [categorias, setCategorias] = useState<Categoria[]>([]);
  useEffect(() => {
    api.listarCategorias().then(setCategorias).catch(() => {});
  }, []);

  return (
    <div className="flex h-full flex-col overflow-auto p-6">
      <Pestanas etiqueta="Reportes" pestanas={PESTANAS} activa={pestana} onCambiar={onPestana} />
      {pestana === "hoy" ? (
        <Hoy activa={activa} onVerBajoMinimo={onVerBajoMinimo} onVerCliente={onVerCliente} onVerProducto={onVerProducto} />
      ) : (
        <div className="space-y-4">
          <SelectorRango rango={rango} onCambiar={setRango} />
          {pestana === "ventas" && <ReporteVentas activa={activa} rango={rango} />}
          {pestana === "productos" && (
            <ReporteProductos activa={activa} rango={rango} categorias={categorias} onVerProducto={onVerProducto} />
          )}
          {pestana === "ganancia" && <ReporteGanancia activa={activa} rango={rango} />}
          {pestana === "turnos" && <ReporteTurnos activa={activa} rango={rango} />}
          {pestana === "fiados" && <ReporteFiados activa={activa} rango={rango} onVerCliente={onVerCliente} />}
        </div>
      )}
    </div>
  );
}
