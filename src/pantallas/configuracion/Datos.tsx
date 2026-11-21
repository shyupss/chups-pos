import { Download, Upload } from "lucide-react";
import { useState, type ReactNode } from "react";
import { api, textoError, type TipoImportacion } from "../../api";
import BotonExportar from "../../componentes/BotonExportar";
import DialogoImportar from "../../componentes/DialogoImportar";
import { avisarCambioStock } from "../../eventos";
import { rangoDe, SelectorRango, type Rango } from "../reportes/Rango";

/** Configuración > Datos: exportar a Excel e importar productos y clientes. */
export default function DatosConfig({ respaldos }: { respaldos?: ReactNode }) {
  const [rango, setRango] = useState<Rango>(() => rangoDe("mes"));
  const [importar, setImportar] = useState<TipoImportacion | null>(null);
  const [aviso, setAviso] = useState<{ texto: string; error?: boolean; ruta?: string } | null>(null);

  const ejemplo = async (tipo: TipoImportacion) => {
    try {
      const ruta = await api.descargarPlanillaEjemplo(tipo);
      if (ruta) setAviso({ texto: "Planilla de ejemplo guardada", ruta });
    } catch (e) {
      setAviso({ texto: textoError(e), error: true });
    }
  };

  return (
    <div className="space-y-6">
      <div className="space-y-3">
        <h3 className="font-medium">Exportar a Excel</h3>
        <div className="flex flex-wrap gap-2">
          <BotonExportar que="productos" etiqueta="Productos" />
          <BotonExportar que="stock" etiqueta="Stock y valorización" />
          <BotonExportar que="clientes" etiqueta="Clientes y deudas" />
        </div>
        <div className="space-y-2 rounded-lg border border-linea p-3">
          <p className="text-sm text-tenue">Ventas, movimientos y compras del rango:</p>
          <SelectorRango rango={rango} onCambiar={setRango} />
          <div className="flex flex-wrap gap-2">
            <BotonExportar que="ventas" desde={rango.desde} hasta={rango.hasta} etiqueta="Ventas con su detalle" />
            <BotonExportar que="movimientos" desde={rango.desde} hasta={rango.hasta} etiqueta="Movimientos de inventario" />
            <BotonExportar que="compras" desde={rango.desde} hasta={rango.hasta} etiqueta="Compras" />
          </div>
        </div>
      </div>

      <div className="space-y-3 border-t border-linea pt-5">
        <h3 className="font-medium">Importar desde Excel</h3>
        <p className="text-sm text-tenue">
          Planillas .xlsx, .xls, .ods o .csv. Antes de guardar se ve qué filas son nuevas, cuáles actualizan y cuáles tienen errores.
        </p>
        {(["productos", "clientes"] as const).map((t) => (
          <div key={t} className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              onClick={() => setImportar(t)}
              className="flex h-9 items-center gap-2 rounded-lg bg-acento px-4 font-semibold text-sobre-acento"
            >
              <Upload size={16} aria-hidden /> Importar {t}
            </button>
            <button
              type="button"
              onClick={() => void ejemplo(t)}
              className="flex h-9 items-center gap-2 rounded-lg border border-linea px-3 hover:bg-fondo"
            >
              <Download size={16} aria-hidden /> Descargar planilla de ejemplo
            </button>
          </div>
        ))}
        {aviso && (
          <p className={`flex items-center gap-2 text-sm ${aviso.error ? "text-peligro" : "text-exito"}`}>
            {aviso.texto}
            {aviso.ruta && (
              <button type="button" onClick={() => void api.abrirArchivo(aviso.ruta!).catch(() => {})} className="font-medium underline">
                Abrir
              </button>
            )}
          </p>
        )}
      </div>

      {respaldos}

      {importar && (
        <DialogoImportar
          tipo={importar}
          onCerrar={() => setImportar(null)}
          onImportado={() => {
            if (importar === "productos") avisarCambioStock();
          }}
        />
      )}
    </div>
  );
}
