import { Upload } from "lucide-react";
import { useState } from "react";
import { api, textoError, type InfoRespaldo } from "../api";
import { fechaHora, tamanoArchivo } from "../formato";
import Boton, { type VarianteBoton } from "./Boton";
import Dialogo from "./Dialogo";
import { esCancelado, useConPin } from "./PedirPin";

/**
 * "Importar una copia…": se elige un archivo (.chups o .db), se muestra de qué fecha es y cuántos productos y ventas
 * trae, se avisa que reemplaza todos los datos de este computador y recién entonces se importa (con el PIN, si hay
 * uno). Antes se hace una copia de lo que hay ahora, y después la app se reinicia. Sirve en Configuración y en el
 * asistente de primer arranque.
 */
export default function ImportarCopia({
  etiqueta = "Importar una copia…",
  variante = "secundario",
  icono = true,
}: {
  etiqueta?: string;
  variante?: VarianteBoton;
  icono?: boolean;
}) {
  const conPin = useConPin();
  const [info, setInfo] = useState<InfoRespaldo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [buscando, setBuscando] = useState(false);

  const elegir = async () => {
    setBuscando(true);
    setError(null);
    try {
      const ruta = await api.elegirRespaldo();
      if (ruta) setInfo(await api.inspeccionarRespaldo(ruta));
    } catch (e) {
      setError(textoError(e));
    } finally {
      setBuscando(false);
    }
  };

  return (
    <>
      <Boton variante={variante} icono={icono ? Upload : undefined} disabled={buscando} onClick={() => void elegir()}>
        {etiqueta}
      </Boton>
      {error && (
        <p role="alert" className="w-full text-sm text-peligro">
          {error}
        </p>
      )}
      {info && (
        <DialogoImportarCopia
          info={info}
          onCerrar={() => setInfo(null)}
          onImportar={() =>
            conPin((pin) => api.restaurarRespaldo(info.ruta, pin), {
              accion: "Importar la copia y reemplazar todos los datos de este computador",
            })
          }
        />
      )}
    </>
  );
}

function DialogoImportarCopia({ info, onCerrar, onImportar }: { info: InfoRespaldo; onCerrar: () => void; onImportar: () => Promise<void> }) {
  const [error, setError] = useState<string | null>(null);
  const [trabajando, setTrabajando] = useState(false);
  const importar = async () => {
    setTrabajando(true);
    setError(null);
    try {
      // Si sale bien, la app se reinicia y este diálogo desaparece.
      await onImportar();
    } catch (e) {
      if (!esCancelado(e)) setError(textoError(e));
      setTrabajando(false);
    }
  };
  const datos: [string, string][] = [
    ["Archivo", info.ruta.split(/[\\/]/).pop() ?? info.ruta],
    ["Es de la fecha", fechaHora(info.fecha)],
    ["Negocio", info.negocio ?? "—"],
    ["Productos", info.productos.toLocaleString("es-CL")],
    ["Ventas", info.ventas.toLocaleString("es-CL")],
    ["Última venta", info.ultimaVenta ? fechaHora(info.ultimaVenta) : "Sin ventas"],
    ["Tamaño", tamanoArchivo(info.bytes)],
  ];
  return (
    <Dialogo titulo="Importar una copia" onCerrar={onCerrar} ancho="w-[34rem]">
      <dl className="mt-3 grid grid-cols-[9rem_1fr] gap-x-4 gap-y-1 tabular-nums">
        {datos.map(([k, v]) => (
          <div key={k} className="contents">
            <dt className="text-tenue">{k}</dt>
            <dd className="min-w-0 break-all">{v}</dd>
          </div>
        ))}
      </dl>
      {info.versionEsquema < info.versionApp && (
        <p className="mt-3 text-sm text-tenue">Es de una versión anterior de CHUPS POS: se pondrá al día al abrirla.</p>
      )}
      <p className="mt-4 rounded-lg bg-aviso-fondo px-3 py-2 text-aviso">
        Esto reemplaza todos los datos de este computador. Antes se hace una copia de lo que hay ahora (queda en la carpeta de copias
        como «antes-de-restaurar») y después CHUPS POS se reinicia.
      </p>
      {error && (
        <p role="alert" className="mt-3 rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">
          {error}
        </p>
      )}
      <div className="mt-5 flex gap-2">
        <Boton variante="terciario" atajo="Esc" onClick={onCerrar} className="flex-1">
          Cancelar
        </Boton>
        <Boton variante="peligro" onClick={() => void importar()} disabled={trabajando} className="flex-[2]">
          {trabajando ? "Importando…" : "Importar y reiniciar"}
        </Boton>
      </div>
    </Dialogo>
  );
}
