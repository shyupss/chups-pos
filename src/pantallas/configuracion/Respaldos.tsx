import Boton from "../../componentes/Boton";
import { FolderOpen, HardDriveDownload, RotateCcw, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { api, textoError, type EstadoRespaldos, type InfoRespaldo } from "../../api";
import { Ajuste, Columnas, Tarjeta } from "../../componentes/Ajustes";
import Dialogo from "../../componentes/Dialogo";
import Opciones, { type Opcion } from "../../componentes/Opciones";
import type { ConfigEditable } from "../../hooks/useConfig";
import { esCancelado, useConPin } from "../../componentes/PedirPin";
import { fechaHora } from "../../formato";
import ExcelTarjeta from "./Datos";

const DIAS_AVISO = 3;
const CONSERVAR: Opcion<string>[] = ["7", "14", "30", "60"].map((n) => ({ id: n, nombre: n }));

export function tamano(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toLocaleString("es-CL", { maximumFractionDigits: 1 })} MB`;
  return `${Math.max(1, Math.round(bytes / 1024)).toLocaleString("es-CL")} KB`;
}

const diasDesde = (iso: string) => Math.floor((Date.now() - new Date(iso).getTime()) / 86_400_000);

/** Configuración > Datos y respaldos: respaldos, la base, Excel, restaurar y la lista de respaldos. */
export default function Respaldos({ cfg }: { cfg: ConfigEditable }) {
  const conPin = useConPin();
  const [estado, setEstado] = useState<EstadoRespaldos | null>(null);
  const [aviso, setAviso] = useState<{ texto: string; error?: boolean } | null>(null);
  const [trabajando, setTrabajando] = useState(false);
  const [restaurar, setRestaurar] = useState<InfoRespaldo | null>(null);

  const leer = useCallback(() => {
    api
      .estadoRespaldos()
      .then(setEstado)
      .catch((e) => setAviso({ texto: textoError(e), error: true }));
  }, []);
  // Al cambiar la carpeta o cuántos se conservan, la lista se vuelve a leer.
  useEffect(leer, [leer, cfg.config?.respaldosCarpeta, cfg.config?.respaldosConservar]);
  const conservar = String(cfg.config?.respaldosConservar ?? 14);

  const accion = async (hacer: () => Promise<string | null>) => {
    setTrabajando(true);
    setAviso(null);
    try {
      const texto = await hacer();
      if (texto) setAviso({ texto });
      leer();
    } catch (e) {
      if (!esCancelado(e)) setAviso({ texto: textoError(e), error: true });
    } finally {
      setTrabajando(false);
    }
  };

  if (!estado) return <p className="text-tenue">Cargando…</p>;
  const ultimo = estado.respaldos[0];
  const viejo = !ultimo || diasDesde(ultimo.fecha) > DIAS_AVISO;

  return (
    <>
      <Columnas
        izquierda={
          <Tarjeta titulo="Respaldos" ayuda="Una copia completa de la base, hecha sin detener la caja.">
            {estado.error && (
              <p className="flex items-start gap-2 rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">
                <TriangleAlert size={18} className="mt-0.5 shrink-0" aria-hidden /> {estado.error}
              </p>
            )}
            <p
              className={`rounded-lg px-3 py-2 ${viejo ? "bg-aviso-fondo text-aviso" : "bg-exito-fondo text-exito"}`}
              role={viejo ? "alert" : undefined}
            >
              {ultimo
                ? `Último respaldo: ${fechaHora(ultimo.fecha)}${viejo ? ` (hace ${diasDesde(ultimo.fecha)} días: conviene respaldar ahora)` : ""}`
                : "Todavía no hay respaldos: conviene respaldar ahora."}
            </p>
            <Ajuste etiqueta="Respaldos automáticos que se conservan" ayuda="Los más viejos se borran al hacer uno nuevo.">
              <Opciones
                nombre="respaldos-conservar"
                etiqueta="Respaldos que se conservan"
                opciones={CONSERVAR.some((o) => o.id === conservar) ? CONSERVAR : [...CONSERVAR, { id: conservar, nombre: conservar }]}
                valor={conservar}
                onCambiar={(v) => void cfg.cambiar({ respaldosConservar: Number(v) })}
              />
            </Ajuste>
            <div className="flex flex-wrap gap-2">
              <Boton
                variante="primario"
                icono={HardDriveDownload}
                disabled={trabajando}
                onClick={() => void accion(async () => `Respaldo guardado: ${(await api.respaldarAhora()).nombre}`)}
              >
                {trabajando ? "Respaldando…" : "Respaldar ahora"}
              </Boton>
              <Boton
                disabled={trabajando}
                onClick={() =>
                  void accion(async () => {
                    const r = await api.respaldarEn();
                    return r ? `Respaldo guardado en ${r.ruta}` : null;
                  })
                }
              >
                Respaldar en…
              </Boton>
              <Boton
                icono={FolderOpen}
                onClick={() => void api.abrirCarpetaRespaldos().catch((e) => setAviso({ texto: textoError(e), error: true }))}
              >
                Abrir carpeta
              </Boton>
            </div>
            {aviso && <p className={`text-sm break-all ${aviso.error ? "text-peligro" : "text-exito"}`}>{aviso.texto}</p>}
          </Tarjeta>
        }
        derecha={
          <>
            <Tarjeta titulo="Base de datos">
              <dl className="grid grid-cols-[9rem_1fr] gap-x-4 gap-y-2 text-sm">
                <dt className="text-tenue">Archivo</dt>
                <dd className="min-w-0 break-all select-text">
                  {estado.rutaBase} <span className="text-tenue">({tamano(estado.bytesBase)})</span>
                </dd>
                <dt className="text-tenue">Carpeta de respaldos</dt>
                <dd className="min-w-0 break-all select-text">
                  {estado.carpeta}
                  <span className="mt-1 -ml-2.5 flex flex-wrap gap-1">
                    <Boton
                      variante="terciario"
                      tamano="chico"
                      onClick={() =>
                        void api.elegirCarpetaRespaldos().then((c) => c && void cfg.cambiar({ respaldosCarpeta: c }))
                      }
                    >
                      Cambiar…
                    </Boton>
                    {cfg.config?.respaldosCarpeta && (
                      <Boton variante="terciario" tamano="chico" onClick={() => void cfg.cambiar({ respaldosCarpeta: null })}>
                        Usar la predeterminada
                      </Boton>
                    )}
                  </span>
                </dd>
              </dl>
            </Tarjeta>
            <ExcelTarjeta />
            <Tarjeta titulo="Restaurar" ayuda="Reemplaza toda la base por la de un respaldo. Siempre pide el PIN, si hay uno.">
              <div>
                <Boton
                  variante="peligro"
                  icono={RotateCcw}
                  disabled={trabajando}
                  onClick={() =>
                    void accion(async () => {
                      const ruta = await api.elegirRespaldo();
                      if (ruta) setRestaurar(await api.inspeccionarRespaldo(ruta));
                      return null;
                    })
                  }
                >
                  Restaurar un respaldo…
                </Boton>
              </div>
            </Tarjeta>
          </>
        }
      />

      <div className="mt-5">
        <Tarjeta titulo="Respaldos guardados" ayuda={`En ${estado.carpeta}`}>
          <div className="overflow-hidden rounded-lg border border-linea">
            <div className="grid grid-cols-[minmax(0,1fr)_10rem_6rem] gap-3 border-b border-linea bg-suave px-3 py-2 text-[13px] font-medium text-tenue">
              <span>Respaldo</span>
              <span>Fecha</span>
              <span className="text-right">Tamaño</span>
            </div>
            {estado.respaldos.length === 0 ? (
              <p className="p-4 text-center text-tenue">No hay respaldos en la carpeta.</p>
            ) : (
              <ul className="max-h-64 overflow-auto">
                {estado.respaldos.map((r) => (
                  <li key={r.ruta} className="grid grid-cols-[minmax(0,1fr)_10rem_6rem] gap-3 border-b border-linea px-3 py-2 text-sm tabular-nums last:border-b-0">
                    <span className="truncate" title={r.ruta}>
                      {r.nombre}
                    </span>
                    <span className="text-tenue">{fechaHora(r.fecha)}</span>
                    <span className="text-right">{tamano(r.bytes)}</span>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </Tarjeta>
      </div>

      {restaurar && (
        <DialogoRestaurar
          info={restaurar}
          onCerrar={() => setRestaurar(null)}
          onRestaurar={() =>
            conPin((pin) => api.restaurarRespaldo(restaurar.ruta, pin), {
              accion: "Restaurar el respaldo y reemplazar la base actual",
            })
          }
        />
      )}
    </>
  );
}

function DialogoRestaurar({
  info,
  onCerrar,
  onRestaurar,
}: {
  info: InfoRespaldo;
  onCerrar: () => void;
  onRestaurar: () => Promise<void>;
}) {
  const [error, setError] = useState<string | null>(null);
  const [trabajando, setTrabajando] = useState(false);
  const restaurar = async () => {
    setTrabajando(true);
    setError(null);
    try {
      // Si sale bien, la app se reinicia y este diálogo desaparece.
      await onRestaurar();
    } catch (e) {
      if (!esCancelado(e)) setError(textoError(e));
      setTrabajando(false);
    }
  };
  const datos: [string, string][] = [
    ["Archivo", info.ruta.split(/[\\/]/).pop() ?? info.ruta],
    ["Fecha del archivo", fechaHora(info.fecha)],
    ["Negocio", info.negocio ?? "—"],
    [
      "Versión de la base",
      info.versionEsquema === info.versionApp
        ? `${info.versionEsquema} (la de esta app)`
        : `${info.versionEsquema} (se pondrá al día a la ${info.versionApp} al abrirla)`,
    ],
    ["Productos", info.productos.toLocaleString("es-CL")],
    ["Ventas", info.ventas.toLocaleString("es-CL")],
    ["Última venta", info.ultimaVenta ? fechaHora(info.ultimaVenta) : "Sin ventas"],
    ["Tamaño", tamano(info.bytes)],
  ];
  return (
    <Dialogo titulo="Restaurar un respaldo" onCerrar={onCerrar} ancho="w-[34rem]">
      <dl className="mt-3 grid grid-cols-[9rem_1fr] gap-x-4 gap-y-1 tabular-nums">
        {datos.map(([k, v]) => (
          <div key={k} className="contents">
            <dt className="text-tenue">{k}</dt>
            <dd className="min-w-0 break-all">{v}</dd>
          </div>
        ))}
      </dl>
      <p className="mt-4 rounded-lg bg-aviso-fondo px-3 py-2 text-aviso">
        Todo lo que se hizo después de este respaldo se reemplaza. Antes se respalda la base actual (queda en la carpeta de
        respaldos como «antes-de-restaurar») y después la app se reinicia.
      </p>
      {error && <p className="mt-3 rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}
      <div className="mt-5 flex gap-2">
        <Boton variante="terciario" atajo="Esc" onClick={onCerrar} className="flex-1">
          Cancelar
        </Boton>
        <Boton variante="peligro" onClick={() => void restaurar()} disabled={trabajando} className="flex-[2]">
          {trabajando ? "Restaurando…" : "Restaurar y reiniciar"}
        </Boton>
      </div>
    </Dialogo>
  );
}
