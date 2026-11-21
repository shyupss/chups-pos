import { FolderOpen, HardDriveDownload, RotateCcw, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { api, textoError, type EstadoRespaldos, type InfoRespaldo } from "../../api";
import Dialogo from "../../componentes/Dialogo";
import Opciones, { type Opcion } from "../../componentes/Opciones";
import type { ConfigEditable } from "../../hooks/useConfig";
import { esCancelado, useConPin } from "../../componentes/PedirPin";
import { fechaHora } from "../../formato";

const DIAS_AVISO = 3;
const CONSERVAR: Opcion<string>[] = ["7", "14", "30", "60"].map((n) => ({ id: n, nombre: n }));

export function tamano(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toLocaleString("es-CL", { maximumFractionDigits: 1 })} MB`;
  return `${Math.max(1, Math.round(bytes / 1024)).toLocaleString("es-CL")} KB`;
}

const diasDesde = (iso: string) => Math.floor((Date.now() - new Date(iso).getTime()) / 86_400_000);

/** Configuración > Datos > Respaldos: la base, el último respaldo, la lista y restaurar. */
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
    <div className="space-y-4">
      <dl className="grid grid-cols-[10rem_1fr] gap-x-4 gap-y-1 text-sm">
        <dt className="text-tenue">Base de datos</dt>
        <dd className="min-w-0 break-all select-text">
          {estado.rutaBase} <span className="text-tenue">({tamano(estado.bytesBase)})</span>
        </dd>
        <dt className="text-tenue">Carpeta de respaldos</dt>
        <dd className="min-w-0 break-all select-text">
          {estado.carpeta}{" "}
          <button
            type="button"
            onClick={() =>
              void api.elegirCarpetaRespaldos().then((c) => c && void cfg.cambiar({ respaldosCarpeta: c }))
            }
            className="text-acento hover:underline"
          >
            Cambiar…
          </button>
          {cfg.config?.respaldosCarpeta && (
            <>
              {" · "}
              <button type="button" onClick={() => void cfg.cambiar({ respaldosCarpeta: null })} className="text-acento hover:underline">
                Usar la predeterminada
              </button>
            </>
          )}
        </dd>
      </dl>
      <div className="flex items-center gap-3 text-sm">
        <span className="text-tenue">Respaldos automáticos que se conservan</span>
        <Opciones
          nombre="respaldos-conservar"
          etiqueta="Respaldos que se conservan"
          opciones={CONSERVAR.some((o) => o.id === conservar) ? CONSERVAR : [...CONSERVAR, { id: conservar, nombre: conservar }]}
          valor={conservar}
          onCambiar={(v) => void cfg.cambiar({ respaldosConservar: Number(v) })}
        />
      </div>

      {estado.error && (
        <p className="flex items-start gap-2 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">
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

      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          disabled={trabajando}
          onClick={() => void accion(async () => `Respaldo guardado: ${(await api.respaldarAhora()).nombre}`)}
          className="flex h-10 items-center gap-2 rounded-lg bg-acento px-4 font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
        >
          <HardDriveDownload size={16} aria-hidden /> {trabajando ? "Respaldando…" : "Respaldar ahora"}
        </button>
        <button
          type="button"
          disabled={trabajando}
          onClick={() =>
            void accion(async () => {
              const r = await api.respaldarEn();
              return r ? `Respaldo guardado en ${r.ruta}` : null;
            })
          }
          className="h-10 rounded-lg border border-linea px-4 hover:bg-fondo"
        >
          Respaldar en…
        </button>
        <button
          type="button"
          onClick={() => void api.abrirCarpetaRespaldos().catch((e) => setAviso({ texto: textoError(e), error: true }))}
          className="flex h-10 items-center gap-2 rounded-lg border border-linea px-4 hover:bg-fondo"
        >
          <FolderOpen size={16} aria-hidden /> Abrir carpeta
        </button>
        <span className="flex-1" />
        <button
          type="button"
          disabled={trabajando}
          onClick={() =>
            void accion(async () => {
              const info = await api.elegirRespaldo();
              if (info) setRestaurar(info);
              return null;
            })
          }
          className="flex h-10 items-center gap-2 rounded-lg border border-linea px-4 text-peligro hover:bg-peligro/10"
        >
          <RotateCcw size={16} aria-hidden /> Restaurar un respaldo…
        </button>
      </div>
      {aviso && <p className={`text-sm break-all ${aviso.error ? "text-peligro" : "text-exito"}`}>{aviso.texto}</p>}

      <div className="overflow-hidden rounded-lg border border-linea">
        <div className="grid grid-cols-[minmax(0,1fr)_10rem_6rem] gap-3 border-b border-linea bg-fondo px-3 py-2 text-xs font-medium uppercase text-tenue">
          <span>Respaldo</span>
          <span>Fecha</span>
          <span className="text-right">Tamaño</span>
        </div>
        {estado.respaldos.length === 0 ? (
          <p className="p-4 text-center text-tenue">No hay respaldos en la carpeta.</p>
        ) : (
          <ul className="max-h-64 overflow-auto">
            {estado.respaldos.map((r) => (
              <li key={r.ruta} className="grid grid-cols-[minmax(0,1fr)_10rem_6rem] gap-3 border-b border-linea px-3 py-2 text-sm tabular-nums">
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
    </div>
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
      {error && <p className="mt-3 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}
      <div className="mt-5 flex gap-2">
        <button type="button" onClick={onCerrar} className="h-11 flex-1 rounded-lg text-tenue hover:bg-fondo hover:text-texto">
          Cancelar <span className="text-xs opacity-60">Esc</span>
        </button>
        <button
          type="button"
          onClick={() => void restaurar()}
          disabled={trabajando}
          className="h-11 flex-[2] rounded-lg bg-peligro font-semibold text-superficie disabled:bg-linea disabled:text-tenue"
        >
          {trabajando ? "Restaurando…" : "Restaurar y reiniciar"}
        </button>
      </div>
    </Dialogo>
  );
}
