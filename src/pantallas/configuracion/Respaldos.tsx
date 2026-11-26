import { ChevronRight, Download, FolderOpen, HardDriveDownload, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { api, textoError, type EstadoRespaldos, type FrecuenciaRespaldos, type Respaldo } from "../../api";
import { Ajuste, Columnas, Tarjeta } from "../../componentes/Ajustes";
import Boton from "../../componentes/Boton";
import DialogoFormatoCopia, { type FormatoCopia } from "../../componentes/DialogoFormatoCopia";
import ImportarCopia from "../../componentes/ImportarCopia";
import OpcionesLista, { type OpcionConDetalle } from "../../componentes/OpcionesLista";
import { esCancelado } from "../../componentes/PedirPin";
import type { ConfigEditable } from "../../hooks/useConfig";
import { carpetaDe, fechaHora, tamanoArchivo } from "../../formato";
import TablaVirtual, { type ColumnaTabla } from "../reportes/TablaVirtual";
import PlanillasExcel from "./Datos";
import { alcanceHorario, copiasAGuardar, estadoDeLaCopia, MAX_COPIAS, MIN_COPIAS } from "./respaldos-nucleo";

const FRECUENCIAS: OpcionConDetalle<FrecuenciaRespaldos>[] = [
  { id: "cadaHora", nombre: "Cada hora", detalle: "Una copia por hora mientras CHUPS POS está abierto." },
  { id: "alCerrarCaja", nombre: "Al cerrar la caja (recomendado)", detalle: "Al cerrar cada turno y, al abrir la app, si ya pasó un día." },
  { id: "unaVezAlDia", nombre: "Una vez al día", detalle: "Al abrir la app o al cerrar la caja, si ya pasó un día." },
  { id: "unaVezALaSemana", nombre: "Una vez a la semana", detalle: "Al abrir la app o al cerrar la caja, si ya pasó una semana." },
  { id: "soloManual", nombre: "Solo manual", detalle: "Solo cuando tú aprietas «Hacer una copia ahora»." },
];

/** "Cuántas copias guardar": un número que se escribe; se guarda al salir del campo o con Enter. */
function CampoCopias({ valor, onCambiar }: { valor: number; onCambiar: (n: number) => void }) {
  const [texto, setTexto] = useState(String(valor));
  useEffect(() => setTexto(String(valor)), [valor]);
  const confirmar = () => {
    const n = copiasAGuardar(texto);
    if (n === null) setTexto(String(valor));
    else if (n !== valor) onCambiar(n);
  };
  return (
    <label className="flex items-center gap-2">
      <input
        inputMode="numeric"
        value={texto}
        onChange={(e) => setTexto(e.target.value.replace(/\D/g, "").slice(0, 3))}
        onBlur={confirmar}
        onKeyDown={(e) => e.key === "Enter" && confirmar()}
        aria-label={`Cuántas copias guardar (de ${MIN_COPIAS} a ${MAX_COPIAS})`}
        className="h-11 w-24 rounded-lg border border-borde-control bg-superficie px-3 text-right focus:border-acento"
      />
      <span className="text-tenue">copias</span>
    </label>
  );
}

type Dialogo = "ahora" | "archivo" | null;
type Aviso = { texto: string; error?: boolean; ruta?: string } | null;

/** Configuración > Datos y respaldos: la copia de seguridad, llevar los datos a otro computador y las planillas Excel. */
export default function Respaldos({ cfg }: { cfg: ConfigEditable }) {
  const [estado, setEstado] = useState<EstadoRespaldos | null>(null);
  const [aviso, setAviso] = useState<Aviso>(null);
  const [trabajando, setTrabajando] = useState(false);
  const [dialogo, setDialogo] = useState<Dialogo>(null);

  const leer = useCallback(() => {
    api
      .estadoRespaldos()
      .then(setEstado)
      .catch((e) => setAviso({ texto: textoError(e), error: true }));
  }, []);
  // Al cambiar la carpeta o cuántas copias se guardan, la lista se vuelve a leer.
  useEffect(leer, [leer, cfg.config?.respaldosCarpeta, cfg.config?.respaldosConservar]);

  const conservar = cfg.config?.respaldosConservar ?? 14;
  const frecuencia = cfg.config?.respaldosFrecuencia ?? "alCerrarCaja";

  /** Hace la copia en el formato elegido: completa (en la carpeta de copias o en un archivo) o planilla Excel. */
  const hacer = async (formato: FormatoCopia, donde: "carpeta" | "archivo") => {
    setTrabajando(true);
    setAviso(null);
    try {
      if (formato === "excel") {
        const ruta = await api.exportarDatos("todo", null, null);
        if (ruta) setAviso({ texto: `Planilla guardada en ${carpetaDe(ruta)}`, ruta });
      } else if (donde === "carpeta") {
        const r = await api.respaldarAhora();
        setAviso({ texto: `Copia hecha: ${r.nombre}` });
      } else {
        const r: Respaldo | null = await api.respaldarEn();
        if (r) setAviso({ texto: `Copia guardada en ${carpetaDe(r.ruta)}`, ruta: r.ruta });
      }
      setDialogo(null);
      leer();
    } catch (e) {
      if (!esCancelado(e)) setAviso({ texto: textoError(e), error: true });
      setDialogo(null);
    } finally {
      setTrabajando(false);
    }
  };

  const columnas = useMemo<ColumnaTabla<Respaldo>[]>(
    () => [
      { titulo: "Copia", ancho: "minmax(0,1fr)", celda: (r) => <span title={r.ruta}>{r.nombre}</span>, orden: (r) => r.nombre },
      { titulo: "Fecha", ancho: "10.5rem", celda: (r) => fechaHora(r.fecha), orden: (r) => r.fecha },
      { titulo: "Tamaño", ancho: "6rem", derecha: true, celda: (r) => tamanoArchivo(r.bytes), orden: (r) => r.bytes },
    ],
    [],
  );

  if (!estado) return <p className="text-tenue">Cargando…</p>;
  const ultima = estado.respaldos[0];
  const { texto, viejo } = estadoDeLaCopia(ultima?.fecha ?? null);

  return (
    <>
      <Columnas
        izquierda={
          <Tarjeta titulo="Copia de seguridad" ayuda="Una copia de todo tu negocio, hecha sin detener la caja.">
            {estado.error && (
              <p role="alert" className="flex items-start gap-2 rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">
                <TriangleAlert size={18} className="mt-0.5 shrink-0" aria-hidden /> {estado.error}
              </p>
            )}
            <p
              className={`rounded-lg px-4 py-3 text-lg font-medium ${viejo ? "bg-aviso-fondo text-aviso" : "bg-exito-fondo text-exito"}`}
              role={viejo ? "alert" : "status"}
            >
              {texto}
            </p>
            <div>
              <Boton variante="primario" icono={HardDriveDownload} disabled={trabajando} onClick={() => setDialogo("ahora")}>
                {trabajando ? "Haciendo la copia…" : "Hacer una copia ahora"}
              </Boton>
            </div>
            {aviso && <Resultado aviso={aviso} />}
            <div>
              <div className="text-sm font-medium">Copias automáticas</div>
              <div className="mb-2 text-xs text-tenue">CHUPS POS las hace solo, sin detener la caja. Siempre son copias completas.</div>
              <OpcionesLista
                nombre="respaldos-frecuencia"
                etiqueta="Cuándo hacer copias automáticas"
                opciones={FRECUENCIAS}
                valor={frecuencia}
                onCambiar={(v) => void cfg.cambiar({ respaldosFrecuencia: v })}
              />
            </div>
            <div className="border-t border-linea pt-4">
              <Ajuste
                etiqueta="Cuántas copias guardar"
                ayuda="Se guardan las últimas; cuando hay más, se borran las más viejas. Las copias hechas antes de importar otra no se borran."
              >
                <CampoCopias valor={conservar} onCambiar={(n) => void cfg.cambiar({ respaldosConservar: n })} />
              </Ajuste>
              {frecuencia === "cadaHora" && (
                <p className="mt-2 rounded-lg bg-info-fondo px-3 py-2 text-[13px] text-info">
                  Con una copia por hora, {conservar} {conservar === 1 ? "copia cubre" : "copias cubren"} {alcanceHorario(conservar)}. Si quieres
                  poder volver más atrás, guarda más copias.
                </p>
              )}
            </div>
            <div className="border-t border-linea pt-4">
              <div className="text-sm font-medium">Las copias se guardan en esta carpeta</div>
              <p className="mt-1 text-sm break-all text-tenue select-text">{estado.carpeta}</p>
              <div className="mt-2 flex flex-wrap gap-2">
                <Boton
                  variante="terciario"
                  tamano="chico"
                  onClick={() => void api.elegirCarpetaRespaldos().then((c) => c && void cfg.cambiar({ respaldosCarpeta: c }))}
                >
                  Cambiar carpeta
                </Boton>
                <Boton
                  variante="terciario"
                  tamano="chico"
                  icono={FolderOpen}
                  onClick={() => void api.abrirCarpetaRespaldos().catch((e) => setAviso({ texto: textoError(e), error: true }))}
                >
                  Abrir carpeta
                </Boton>
                {cfg.config?.respaldosCarpeta && (
                  <Boton variante="terciario" tamano="chico" onClick={() => void cfg.cambiar({ respaldosCarpeta: null })}>
                    Usar la carpeta de siempre
                  </Boton>
                )}
              </div>
            </div>
            <details className="group rounded-lg border border-linea">
              <summary className="flex h-11 cursor-pointer list-none items-center gap-2 px-3 font-medium marker:hidden [&::-webkit-details-marker]:hidden">
                <ChevronRight size={18} aria-hidden className="text-tenue group-open:rotate-90" />
                Ver copias guardadas ({estado.respaldos.length})
              </summary>
              <div className="border-t border-linea p-3">
                <TablaVirtual
                  etiqueta="Copias guardadas"
                  filas={estado.respaldos}
                  clave={(r) => r.ruta}
                  columnas={columnas}
                  vacia="Todavía no hay copias en esta carpeta."
                  alto={320}
                />
              </div>
            </details>
          </Tarjeta>
        }
        derecha={
          <>
            <Tarjeta titulo="Llevar tus datos a otro computador" ayuda="Para cambiarte de equipo o guardar una copia en un pendrive.">
              <div className="flex flex-wrap gap-2">
                <Boton icono={Download} disabled={trabajando} onClick={() => setDialogo("archivo")}>
                  Exportar una copia a un archivo…
                </Boton>
                <ImportarCopia />
              </div>
              <p className="text-sm text-tenue">
                Importar una copia reemplaza todos los datos de este computador y siempre pide el PIN, si hay uno.
              </p>
              <p className="rounded-lg bg-info-fondo px-3 py-2 text-info">
                En el computador nuevo: instala CHUPS POS, elige «Importar una copia» y pide al vendedor una licencia para ese equipo (la
                licencia está atada a cada computador).
              </p>
            </Tarjeta>
            <PlanillasExcel />
          </>
        }
      />

      {dialogo && (
        <DialogoFormatoCopia
          titulo={dialogo === "ahora" ? "Hacer una copia ahora" : "Exportar una copia a un archivo"}
          boton={dialogo === "ahora" ? "Hacer la copia" : "Elegir dónde guardarla…"}
          ocupado={trabajando}
          onCerrar={() => setDialogo(null)}
          onElegir={(formato) => void hacer(formato, dialogo === "ahora" ? "carpeta" : "archivo")}
        />
      )}
    </>
  );
}

/** El resultado de la última acción: en verde (o rojo si falló) y, si hay un archivo, con "Abrir" y "Mostrar en la carpeta". */
function Resultado({ aviso }: { aviso: NonNullable<Aviso> }) {
  return (
    <div role={aviso.error ? "alert" : "status"} className={`flex flex-wrap items-center gap-2 text-sm ${aviso.error ? "text-peligro" : "text-exito"}`}>
      <span className="min-w-0 break-all">{aviso.texto}</span>
      {aviso.ruta && (
        <>
          <Boton variante="terciario" tamano="chico" onClick={() => void api.mostrarEnCarpeta(aviso.ruta!).catch(() => {})}>
            Mostrar en la carpeta
          </Boton>
        </>
      )}
    </div>
  );
}
