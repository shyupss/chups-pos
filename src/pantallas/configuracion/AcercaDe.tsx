import { Copy, Download, FolderOpen, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { api, textoError, type EstadoActualizacion, type EstadoSistema } from "../../api";
import { Columnas, Tarjeta } from "../../componentes/Ajustes";
import { tamanoArchivo } from "../../formato";
import Boton from "../../componentes/Boton";
import registroDeCambios from "../../../CHANGELOG.md?raw";

/** Las notas de una versión: el apartado "## [X.Y.Z]" del CHANGELOG que se empaqueta con la app. */
function notasDe(version: string): string | null {
  const lineas = registroDeCambios.split(/\r?\n/);
  const inicio = lineas.findIndex((l) => new RegExp(`^##\\s+\\[?v?${version.replace(/\./g, "\\.")}\\]?`).test(l));
  if (inicio < 0) return null;
  const fin = lineas.findIndex((l, i) => i > inicio && /^##\s/.test(l));
  return lineas
    .slice(inicio + 1, fin < 0 ? undefined : fin)
    .join("\n")
    .trim();
}

/** Markdown mínimo de las notas: títulos "###" y viñetas "- ". El resto, como texto. */
function Notas({ texto }: { texto: string }) {
  return (
    <div className="flex flex-col gap-1 text-[13px] select-text">
      {texto.split(/\r?\n/).map((l, i) =>
        l.startsWith("### ") ? (
          <h4 key={i} className="mt-2 font-semibold first:mt-0">
            {l.slice(4)}
          </h4>
        ) : l.startsWith("- ") ? (
          <p key={i} className="pl-3 text-tenue before:mr-1.5 before:content-['•']">
            {l.slice(2).replace(/`/g, "")}
          </p>
        ) : l.trim() ? (
          <p key={i} className="text-tenue">
            {l}
          </p>
        ) : null,
      )}
    </div>
  );
}

const fecha = (segundos: number | null) =>
  segundos === null
    ? "Todavía no se buscó"
    : new Date(segundos * 1000).toLocaleString("es-CL", { dateStyle: "long", timeStyle: "short" });

interface Props {
  actualizacion: EstadoActualizacion | null;
  onActualizacion: (e: EstadoActualizacion) => void;
  /** Hay un turno de caja abierto: no se instala ahora, solo al cerrarlo. */
  hayTurno: boolean;
  /** Se pidió instalar y empezó (la app lo muestra y se cerrará). */
  onInstalando: () => void;
}

/** Configuración > Acerca de: datos del sistema y actualizaciones. */
export default function AcercaDe({ actualizacion, onActualizacion, hayTurno, onInstalando }: Props) {
  const [estado, setEstado] = useState<EstadoSistema | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [buscando, setBuscando] = useState(false);
  const [mensaje, setMensaje] = useState<{ error: boolean; texto: string } | null>(null);
  const [soporte, setSoporte] = useState<{ error: boolean; texto: string } | null>(null);

  const [bytesBase, setBytesBase] = useState<number | null>(null);
  useEffect(() => {
    api.estadoSistema().then(setEstado).catch((e) => setError(String(e)));
    api.estadoRespaldos().then((r) => setBytesBase(r.bytesBase)).catch(() => {});
  }, []);

  const buscar = async () => {
    setBuscando(true);
    setMensaje(null);
    try {
      const nuevo = await api.buscarActualizacion();
      onActualizacion(nuevo);
      setMensaje(
        nuevo.disponible ? null : { error: false, texto: `Estás al día: la versión ${nuevo.versionActual} es la más reciente.` },
      );
    } catch (e) {
      setMensaje({ error: true, texto: textoError(e) });
    } finally {
      setBuscando(false);
    }
  };

  const instalarAlCerrar = async (si: boolean) => {
    try {
      onActualizacion(await api.instalarAlCerrarTurno(si));
    } catch (e) {
      setMensaje({ error: true, texto: textoError(e) });
    }
  };

  const instalarAhora = async () => {
    setMensaje(null);
    onInstalando();
    try {
      await api.instalarActualizacion();
    } catch (e) {
      setMensaje({ error: true, texto: textoError(e) });
    }
  };

  const copiarSoporte = async () => {
    setSoporte(null);
    try {
      await navigator.clipboard.writeText(await api.informacionSoporte());
      setSoporte({ error: false, texto: "Información copiada: pégala en el mensaje a soporte." });
    } catch (e) {
      setSoporte({ error: true, texto: textoError(e) });
    }
  };

  const abrirRegistros = async () => {
    setSoporte(null);
    try {
      await api.abrirCarpetaRegistros();
    } catch (e) {
      setSoporte({ error: true, texto: textoError(e) });
    }
  };

  const datos: [string, string | number][] = estado
    ? [
        ["Versión", estado.version],
        ["Esquema de base de datos", `v${estado.versionEsquema}`],
        ["Arranque del backend", `${estado.msInicio} ms`],
        ["Productos", estado.totalProductos],
        ["Base de datos (archivo)", estado.rutaBaseDatos],
        ["Tamaño de la base de datos", bytesBase === null ? "…" : tamanoArchivo(bytesBase)],
      ]
    : [];
  const nueva = actualizacion?.disponible ?? null;
  const notasActuales = estado ? notasDe(estado.version) : null;

  return (
    <Columnas
      izquierda={
        <Tarjeta
          titulo="Actualizaciones"
          ayuda="Se busca una vez al día, solo con internet y nunca con un turno abierto. Antes de instalar se hace un respaldo."
        >
          <dl className="grid grid-cols-[12rem_1fr] gap-x-4 gap-y-2">
            <dt className="text-tenue">Versión instalada</dt>
            <dd>{actualizacion?.versionActual ?? estado?.version ?? "—"}</dd>
            <dt className="text-tenue">Última búsqueda</dt>
            <dd>{fecha(actualizacion?.ultimaBusqueda ?? null)}</dd>
          </dl>
          <div>
            <Boton icono={RefreshCw} disabled={buscando} onClick={() => void buscar()}>
              {buscando ? "Buscando…" : "Buscar actualizaciones"}
            </Boton>
          </div>
          {mensaje && (
            <p role={mensaje.error ? "alert" : "status"} className={mensaje.error ? "text-peligro" : "text-exito"}>
              {mensaje.texto}
            </p>
          )}
          {nueva && (
            <div className="flex flex-col gap-3 border-t border-linea pt-4">
              <p className="w-fit rounded-lg bg-info-fondo px-3 py-2 font-medium text-info">
                Hay una versión nueva: {nueva.version}
              </p>
              {nueva.notas && (
                <div className="max-h-64 overflow-y-auto rounded-lg border border-linea bg-suave p-3">
                  <Notas texto={nueva.notas} />
                </div>
              )}
              {actualizacion?.instalarAlCerrar ? (
                <div className="flex flex-wrap items-center gap-2">
                  <p className="text-exito">Se instalará cuando cierres el turno de caja.</p>
                  <Boton variante="terciario" tamano="chico" onClick={() => void instalarAlCerrar(false)}>
                    Cancelar
                  </Boton>
                </div>
              ) : (
                <div className="flex flex-wrap items-center gap-2">
                  <Boton
                    variante="primario"
                    icono={Download}
                    disabled={hayTurno}
                    title={hayTurno ? "Cierra el turno de caja para instalar ahora" : undefined}
                    onClick={() => void instalarAhora()}
                  >
                    Instalar ahora
                  </Boton>
                  {hayTurno && <Boton onClick={() => void instalarAlCerrar(true)}>Instalar al cerrar el turno</Boton>}
                </div>
              )}
              <p className="text-xs text-tenue">
                Al instalar, CHUPS POS se cierra y vuelve a abrirse. Tus datos no se tocan.
              </p>
            </div>
          )}
        </Tarjeta>
      }
      derecha={
        <>
          <Tarjeta titulo="Información para soporte" ayuda="Datos del sistema, para cuando pidas ayuda.">
            {error ? (
              <p className="text-peligro">No se pudo leer el estado del sistema: {error}</p>
            ) : !estado ? (
              <p className="text-tenue">Cargando…</p>
            ) : (
              <dl className="grid grid-cols-[12rem_1fr] gap-x-4 gap-y-2 tabular-nums">
                {datos.map(([etiqueta, valor]) => (
                  <div key={etiqueta} className="contents">
                    <dt className="text-tenue">{etiqueta}</dt>
                    <dd className="min-w-0 break-all select-text">{valor}</dd>
                  </div>
                ))}
              </dl>
            )}
          </Tarjeta>
          <Tarjeta
            titulo="Soporte"
            ayuda="Si algo falla, copia esta información y mándala junto con el problema. No lleva datos de tus clientes, ni tu PIN ni la clave de la licencia."
          >
            <div className="flex flex-wrap gap-2">
              <Boton icono={Copy} onClick={() => void copiarSoporte()}>
                Copiar información para soporte
              </Boton>
              <Boton icono={FolderOpen} onClick={() => void abrirRegistros()}>
                Abrir carpeta de registros
              </Boton>
            </div>
            {soporte && (
              <p role={soporte.error ? "alert" : "status"} className={soporte.error ? "text-peligro" : "text-exito"}>
                {soporte.texto}
              </p>
            )}
          </Tarjeta>
          {notasActuales && estado && (
            <Tarjeta titulo={`Novedades de la versión ${estado.version}`}>
              <Notas texto={notasActuales} />
            </Tarjeta>
          )}
        </>
      }
    />
  );
}
