import { FileSpreadsheet } from "lucide-react";
import { useState } from "react";
import { api, textoError, type Exportacion } from "../api";

/** Exporta a Excel con "Guardar como" y deja la ruta con "Abrir". */
export default function BotonExportar({
  que,
  desde = null,
  hasta = null,
  etiqueta = "Exportar a Excel",
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
      <button
        type="button"
        onClick={() => void exportar()}
        disabled={estado?.trabajando}
        className="flex h-9 items-center gap-2 rounded-lg border border-linea px-3 hover:bg-fondo disabled:opacity-60"
      >
        <FileSpreadsheet size={16} aria-hidden /> {estado?.trabajando ? "Exportando…" : etiqueta}
      </button>
      {estado?.ruta && (
        <span className="flex items-center gap-2 text-sm text-exito">
          <span className="max-w-64 truncate" title={estado.ruta}>
            Guardado
          </span>
          <button type="button" onClick={() => void api.abrirArchivo(estado.ruta!).catch(() => {})} className="font-medium underline">
            Abrir
          </button>
        </span>
      )}
      {estado?.error && <span className="text-sm text-peligro">{estado.error}</span>}
    </span>
  );
}
