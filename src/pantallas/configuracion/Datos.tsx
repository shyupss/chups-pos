import { Download, Upload } from "lucide-react";
import { useState } from "react";
import { api, textoError, type TipoImportacion } from "../../api";
import { Tarjeta } from "../../componentes/Ajustes";
import Boton from "../../componentes/Boton";
import BotonExportar from "../../componentes/BotonExportar";
import DialogoImportar from "../../componentes/DialogoImportar";
import { avisarCambioStock } from "../../eventos";
import { rangoDe, SelectorRango, type Rango } from "../reportes/Rango";

/** Configuración > Datos y respaldos > Excel: exportar e importar productos y clientes. */
export default function ExcelTarjeta() {
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
    <Tarjeta titulo="Excel" ayuda="Exporta tus datos o impórtalos desde una planilla .xlsx, .xls, .ods o .csv.">
      <div className="flex flex-col gap-2">
        <h3 className="text-xs font-semibold text-tenue">Exportar</h3>
        <div className="flex flex-wrap gap-2">
          <BotonExportar que="productos" etiqueta="Productos" />
          <BotonExportar que="stock" etiqueta="Stock y valorización" />
          <BotonExportar que="clientes" etiqueta="Clientes y deudas" />
        </div>
        <div className="flex flex-col gap-2 rounded-lg border border-linea bg-suave p-3">
          <p className="text-[13px] text-tenue">Ventas, movimientos y compras del rango:</p>
          <SelectorRango rango={rango} onCambiar={setRango} />
          <div className="flex flex-wrap gap-2">
            <BotonExportar que="ventas" desde={rango.desde} hasta={rango.hasta} etiqueta="Ventas con su detalle" />
            <BotonExportar que="movimientos" desde={rango.desde} hasta={rango.hasta} etiqueta="Movimientos de inventario" />
            <BotonExportar que="compras" desde={rango.desde} hasta={rango.hasta} etiqueta="Compras" />
          </div>
        </div>
      </div>

      <div className="flex flex-col gap-2 border-t border-linea pt-4">
        <h3 className="text-xs font-semibold text-tenue">Importar</h3>
        <p className="text-[13px] text-tenue">
          Antes de guardar se ve qué filas son nuevas, cuáles actualizan y cuáles tienen errores.
        </p>
        {(["productos", "clientes"] as const).map((t) => (
          <div key={t} className="flex flex-wrap items-center gap-2">
            <Boton icono={Upload} onClick={() => setImportar(t)}>
              Importar {t}
            </Boton>
            <Boton variante="terciario" icono={Download} onClick={() => void ejemplo(t)}>
              Planilla de ejemplo
            </Boton>
          </div>
        ))}
        {aviso && (
          <p className={`flex items-center gap-2 text-sm ${aviso.error ? "text-peligro" : "text-exito"}`}>
            {aviso.texto}
            {aviso.ruta && (
              <Boton variante="enlace" onClick={() => void api.abrirArchivo(aviso.ruta!).catch(() => {})}>
                Abrir
              </Boton>
            )}
          </p>
        )}
      </div>

      {importar && (
        <DialogoImportar
          tipo={importar}
          onCerrar={() => setImportar(null)}
          onImportado={() => {
            if (importar === "productos") avisarCambioStock();
          }}
        />
      )}
    </Tarjeta>
  );
}
