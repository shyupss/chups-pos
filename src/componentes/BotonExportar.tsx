import Boton from "./Boton";
import { FileSpreadsheet } from "lucide-react";
import { useState } from "react";
import { api, textoError, type Exportacion } from "../api";
import { carpetaDe } from "../formato";

/** Descarga una planilla Excel con "Guardar como" y dice dónde quedó, con "Abrir" y "Mostrar en la carpeta". */
export default function BotonExportar({
  que,
  desde = null,
  hasta = null,
  etiqueta = "Descargar en Excel",
}: {
  que: Exportacion;
  desde?: string | null;
  hasta?: string | null;
  etiqueta?: string;
}) {
  const [estado, setEstado] = useState<{ ruta?: string; error?: string; trabajando?: boolean } | null>(null);
  const exportar = async () => {
    setEstado({ trabajando: true });
    try {
      const ruta = await api.exportarDatos(que, desde, hasta);
      setEstado(ruta ? { ruta } : null);
    } catch (e) {
      setEstado({ error: textoError(e) });
    }
  };
  return (
    <span className="inline-flex flex-wrap items-center gap-2">
      <Boton
        icono={FileSpreadsheet}
        onClick={() => void exportar()}
        disabled={estado?.trabajando}
        title="Planilla Excel: para revisar los números o mandarlos al contador"
      >
        {estado?.trabajando ? "Descargando…" : etiqueta}
      </Boton>
      {estado?.ruta && (
        <span role="status" className="flex flex-wrap items-center gap-2 text-sm text-exito">
          <span className="max-w-64 truncate" title={estado.ruta}>
            Guardado en {carpetaDe(estado.ruta)}
          </span>
          <Boton variante="terciario" tamano="chico" onClick={() => void api.abrirArchivo(estado.ruta!).catch(() => {})}>
            Abrir
          </Boton>
          <Boton variante="terciario" tamano="chico" onClick={() => void api.mostrarEnCarpeta(estado.ruta!).catch(() => {})}>
            Mostrar en la carpeta
          </Boton>
        </span>
      )}
      {estado?.error && <span className="text-sm text-peligro">{estado.error}</span>}
    </span>
  );
}
