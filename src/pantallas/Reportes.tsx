import { useEffect, useState } from "react";
import { api, type Categoria } from "../api";
import Pestanas from "../componentes/Pestanas";
import Hoy from "./reportes/Hoy";
import { ReporteFiados, ReporteGanancia, ReporteProductos, ReporteTurnos, ReporteVentas } from "./reportes/PorRango";
import { rangoDe, SelectorRango, SlotExportar, TituloPeriodo, type Rango } from "./reportes/Rango";
import Resumen from "./reportes/Resumen";

export type PestanaReportes = "resumen" | "hoy" | "ventas" | "productos" | "ganancia" | "turnos" | "fiados";

const PESTANAS: { id: PestanaReportes; nombre: string }[] = [
  { id: "resumen", nombre: "Resumen" },
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

/** Reportes (F6): el Resumen del mes, Hoy y los reportes por período. F6 siempre abre el Resumen; en un reporte, Ctrl+E descarga el Excel. */
export default function Reportes({ activa, pestana, onPestana, onVerBajoMinimo, onVerCliente, onVerProducto }: Props) {
  // El rango se comparte entre pestañas: cambiar de Ventas a Ganancia mira el mismo período.
  const [rango, setRango] = useState<Rango>(() => rangoDe("mes"));
  const [categorias, setCategorias] = useState<Categoria[]>([]);
  // Donde cada reporte dibuja su botón "Descargar este reporte" (la fila del título).
  const [slot, setSlot] = useState<HTMLElement | null>(null);
  useEffect(() => {
    api.listarCategorias().then(setCategorias).catch(() => {});
  }, []);

  return (
    <div className="flex h-full flex-col overflow-auto p-6">
      <Pestanas etiqueta="Reportes" pestanas={PESTANAS} activa={pestana} onCambiar={onPestana} />
      {pestana === "hoy" ? (
        <Hoy activa={activa} onVerBajoMinimo={onVerBajoMinimo} onVerCliente={onVerCliente} onVerProducto={onVerProducto} />
      ) : (
        <SlotExportar.Provider value={slot}>
        <div className="space-y-4">
          <SelectorRango rango={rango} onCambiar={setRango} />
          <div className="flex flex-wrap items-start justify-between gap-3">
            <TituloPeriodo rango={rango} />
            <div ref={setSlot} />
          </div>
          {pestana === "resumen" && <Resumen activa={activa} rango={rango} onVerProductos={() => onPestana("productos")} />}
          {pestana === "ventas" && <ReporteVentas activa={activa} rango={rango} />}
          {pestana === "productos" && (
            <ReporteProductos activa={activa} rango={rango} categorias={categorias} onVerProducto={onVerProducto} />
          )}
          {pestana === "ganancia" && <ReporteGanancia activa={activa} rango={rango} />}
          {pestana === "turnos" && <ReporteTurnos activa={activa} rango={rango} />}
          {pestana === "fiados" && <ReporteFiados activa={activa} rango={rango} onVerCliente={onVerCliente} />}
        </div>
        </SlotExportar.Provider>
      )}
    </div>
  );
}
