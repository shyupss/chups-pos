import { Download, Upload } from "lucide-react";
import { useState } from "react";
import { api, textoError, type TipoImportacion } from "../../api";
import { Tarjeta } from "../../componentes/Ajustes";
import Boton from "../../componentes/Boton";
import BotonExportar from "../../componentes/BotonExportar";
import DialogoImportar from "../../componentes/DialogoImportar";
import { avisarCambioStock } from "../../eventos";
import { carpetaDe } from "../../formato";
import { rangoDe, SelectorRango, type Rango } from "../reportes/Rango";

const IMPORTACIONES: { tipo: TipoImportacion; boton: string; explicacion: string }[] = [
  {
    tipo: "productos",
    boton: "Importar productos…",
    explicacion: "Productos: carga o actualiza tu lista de productos (código, nombre, precio, costo, stock…) desde una planilla.",
  },
  {
    tipo: "clientes",
    boton: "Importar clientes…",
    explicacion: "Clientes del fiado: carga tus clientes con su teléfono, RUT, dirección y límite de crédito. No carga deudas.",
  },
];

/** Configuración > Datos y respaldos > Planillas Excel: exportar (también todo en un libro) e importar productos y clientes. */
export default function PlanillasExcel() {
  const [rango, setRango] = useState<Rango>(() => rangoDe("mes"));
  const [importar, setImportar] = useState<TipoImportacion | null>(null);
  const [aviso, setAviso] = useState<{ texto: string; error?: boolean; ruta?: string } | null>(null);

  const ejemplo = async (tipo: TipoImportacion) => {
    try {
      const ruta = await api.descargarPlanillaEjemplo(tipo);
      if (ruta) setAviso({ texto: `Planilla de ejemplo guardada en ${carpetaDe(ruta)}`, ruta });
    } catch (e) {
      setAviso({ texto: textoError(e), error: true });
    }
  };

  return (
    <Tarjeta titulo="Planillas Excel" ayuda="Pasa tus datos a Excel, o cárgalos desde una planilla (.xlsx, .xls, .ods o .csv).">
      <div className="flex flex-col gap-3">
        <h3 className="text-sm font-semibold">Exportar</h3>
        <div className="flex flex-col gap-1">
          <div>
            <BotonExportar que="todo" etiqueta="Exportar todo a Excel" />
          </div>
          <p className="text-sm text-tenue">
            Un solo libro con una hoja por cada tabla importante (productos, stock, clientes, ventas, movimientos y compras). Se hace en segundo
            plano: puedes seguir vendiendo.
          </p>
        </div>
        <p className="text-sm text-tenue">O solo una lista:</p>
        <div className="flex flex-wrap gap-2">
          <BotonExportar que="productos" etiqueta="Productos" />
          <BotonExportar que="stock" etiqueta="Stock y valorización" />
          <BotonExportar que="clientes" etiqueta="Clientes y deudas" />
        </div>
        <div className="flex flex-col gap-2 rounded-lg border border-linea bg-suave p-3">
          <p className="text-sm text-tenue">Ventas, movimientos y compras de un período:</p>
          <SelectorRango rango={rango} onCambiar={setRango} />
          <div className="flex flex-wrap gap-2">
            <BotonExportar que="ventas" desde={rango.desde} hasta={rango.hasta} etiqueta="Ventas con su detalle" />
            <BotonExportar que="movimientos" desde={rango.desde} hasta={rango.hasta} etiqueta="Movimientos de inventario" />
            <BotonExportar que="compras" desde={rango.desde} hasta={rango.hasta} etiqueta="Compras" />
          </div>
        </div>
      </div>

      <div className="flex flex-col gap-3 border-t border-linea pt-4">
        <h3 className="text-sm font-semibold">Importar</h3>
        <p className="text-sm text-tenue">Antes de guardar ves qué filas son nuevas, cuáles actualizan algo que ya tienes y cuáles tienen errores.</p>
        {IMPORTACIONES.map((i) => (
          <div key={i.tipo} className="flex flex-col gap-2 rounded-lg border border-linea p-3">
            <p>{i.explicacion}</p>
            <div className="flex flex-wrap items-center gap-2">
              <Boton icono={Upload} onClick={() => setImportar(i.tipo)}>
                {i.boton}
              </Boton>
              <Boton variante="terciario" icono={Download} onClick={() => void ejemplo(i.tipo)}>
                Descargar planilla de ejemplo
              </Boton>
            </div>
          </div>
        ))}
        {aviso && (
          <p role={aviso.error ? "alert" : "status"} className={`flex flex-wrap items-center gap-2 text-sm ${aviso.error ? "text-peligro" : "text-exito"}`}>
            <span className="break-all">{aviso.texto}</span>
            {aviso.ruta && (
              <>
                <Boton variante="terciario" tamano="chico" onClick={() => void api.abrirArchivo(aviso.ruta!).catch(() => {})}>
                  Abrir
                </Boton>
                <Boton variante="terciario" tamano="chico" onClick={() => void api.mostrarEnCarpeta(aviso.ruta!).catch(() => {})}>
                  Mostrar en la carpeta
                </Boton>
              </>
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
