import type { ReactNode } from "react";
import type { VentaConfirmada } from "../api";
import { folio, pesos } from "../formato";
import type { SalidaVenta, Tarea } from "../hooks/useSalidasVenta";

interface Acciones {
  reimprimir: () => void;
  /** Repite la impresión que falló (la original o la copia). */
  reintentar: () => void;
  guardarPdf: () => void;
  abrirPdf: () => void;
  cerrar: () => void;
}

interface Props extends Acciones {
  /** La venta recién cobrada; null si el aviso es solo de la boleta de una venta anterior. */
  venta: VentaConfirmada | null;
  salida: SalidaVenta | null;
}

const claseBoton =
  "h-8 shrink-0 rounded-lg border border-exito/40 px-3 font-medium hover:bg-exito/10 disabled:opacity-50";

/** Aviso de la venta registrada, con la impresión, el PDF y el cajón. */
export default function AvisoVenta({ venta, salida, ...acciones }: Props) {
  const imprimiendo = salida?.impresion?.estado === "trabajando";
  const guardando = salida?.pdf?.estado === "trabajando";

  if (venta) {
    return (
      <div className="mt-3 rounded-lg bg-exito-fondo px-4 py-3 text-exito">
        <div className="flex flex-wrap items-center gap-x-4 gap-y-2 tabular-nums">
          <span className="font-semibold">Venta {folio(venta.folio)} registrada</span>
          <span>Total {pesos(venta.total)}</span>
          {venta.medioPago === "efectivo" && venta.vuelto !== null && (
            <span className="text-lg font-semibold">Vuelto {pesos(venta.vuelto)}</span>
          )}
          {venta.medioPago === "fiado" && venta.cliente && (
            <span className="font-semibold">
              Fiado a {venta.cliente}
              {venta.deudaCliente !== null && ` · ahora debe ${pesos(venta.deudaCliente)}`}
            </span>
          )}
          <span className="ml-auto flex gap-2">
            <button type="button" onClick={acciones.reimprimir} disabled={imprimiendo} className={claseBoton} title="Reimprimir (Ctrl+P)">
              Reimprimir
            </button>
            <button type="button" onClick={acciones.guardarPdf} disabled={guardando} className={claseBoton}>
              Guardar PDF
            </button>
          </span>
        </div>
        <Estados salida={salida} acciones={acciones} />
      </div>
    );
  }

  if (!salida) return null;
  return (
    <div className="mt-3 rounded-lg border border-linea bg-superficie px-4 py-3">
      <div className="flex items-center gap-3">
        <span className="font-medium">
          {salida.folio !== null ? `Boleta ${folio(salida.folio)}` : "Última boleta"}
        </span>
        <button
          type="button"
          onClick={acciones.cerrar}
          aria-label="Cerrar aviso"
          className="ml-auto h-7 rounded-lg px-2 text-tenue hover:bg-fondo hover:text-texto"
        >
          ×
        </button>
      </div>
      <Estados salida={salida} acciones={acciones} />
    </div>
  );
}

function Estados({ salida, acciones }: { salida: SalidaVenta | null; acciones: Acciones }) {
  if (!salida) return null;
  const filas: ReactNode[] = [];
  const fila = (clave: string, tarea: Tarea, textos: { trabajando: string; error: string }, accion?: ReactNode) => {
    if (tarea.estado === "error") {
      filas.push(
        <div key={clave} role="alert" className="flex flex-wrap items-center gap-3 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">
          <span className="min-w-0 flex-1">
            {textos.error}: {tarea.texto}
          </span>
          {accion}
        </div>,
      );
    } else {
      filas.push(
        <div key={clave} role="status" className="flex flex-wrap items-center gap-3">
          <span className="min-w-0 flex-1 break-all">
            {tarea.estado === "trabajando" ? textos.trabajando : tarea.texto}
          </span>
          {tarea.estado === "ok" && accion}
        </div>,
      );
    }
  };

  if (salida.impresion) {
    fila(
      "impresion",
      salida.impresion,
      { trabajando: "Imprimiendo la boleta…", error: "No se imprimió la boleta (la venta quedó guardada)" },
      salida.impresion.estado === "error" ? (
        <button type="button" onClick={acciones.reintentar} className="h-8 rounded-lg bg-peligro px-3 font-medium text-superficie">
          Reintentar
        </button>
      ) : undefined,
    );
  }
  if (salida.pdf) {
    const pdf =
      salida.pdf.estado === "ok" && salida.rutaPdf
        ? ({ estado: "ok", texto: `PDF guardado en ${salida.rutaPdf}` } as const)
        : salida.pdf;
    fila(
      "pdf",
      pdf,
      { trabajando: "Guardando el PDF…", error: "No se guardó el PDF" },
      pdf.estado === "ok" ? (
        <button type="button" onClick={acciones.abrirPdf} className={claseBoton}>
          Abrir
        </button>
      ) : undefined,
    );
  }
  if (salida.errorCajon) {
    fila("cajon", { estado: "error", texto: salida.errorCajon }, { trabajando: "", error: "No se abrió el cajón" });
  }
  return filas.length > 0 ? <div className="mt-2 space-y-1 text-sm">{filas}</div> : null;
}
