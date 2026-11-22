import {
  BadgeCheck,
  DatabaseBackup,
  Info,
  Monitor,
  Moon,
  Palette,
  Printer,
  ShieldCheck,
  Store,
  Sun,
  Wallet,
  Wand2,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { api, type EstadoLicencia, type EstadoSistema } from "../api";
import { Ajuste, Columnas, Tarjeta } from "../componentes/Ajustes";
import type { ModoBarra } from "../componentes/BarraLateral";
import Boton from "../componentes/Boton";
import Interruptor from "../componentes/Interruptor";
import Opciones, { type Opcion } from "../componentes/Opciones";
import { useConPinRecordado } from "../componentes/PedirPin";
import LicenciaConfig from "./configuracion/Licencia";
import RespaldosConfig from "./configuracion/Respaldos";
import EscanerConfig from "./configuracion/Escaner";
import ImpresoraConfig from "./configuracion/Impresora";
import NegocioConfig from "./configuracion/Negocio";
import SeguridadConfig from "./configuracion/Seguridad";
import { useConfig, type ConfigEditable } from "../hooks/useConfig";
import { useTema, type Tema } from "../hooks/useTema";

interface Props {
  modoBarra: ModoBarra;
  onModoBarra: (modo: ModoBarra) => void;
  /** Cambió la impresora o se probó: la barra lateral vuelve a leer su estado. */
  onCambioImpresora: () => void;
  /** Estado de la licencia (lo lee App) y el aviso de que cambió al activarla. */
  licencia: EstadoLicencia | null;
  onCambioLicencia: (e: EstadoLicencia) => void;
  /** Vuelve a abrir el asistente de primer arranque. */
  onAbrirAsistente: () => void;
}

export type IdSeccion = "negocio" | "apariencia" | "caja" | "impresora" | "seguridad" | "datos" | "licencia" | "acerca";

/** El orden de la lista lateral. Una sección futura es una entrada más y su componente. */
const SECCIONES: { id: IdSeccion; nombre: string; Icono: LucideIcon }[] = [
  { id: "negocio", nombre: "Negocio", Icono: Store },
  { id: "apariencia", nombre: "Apariencia", Icono: Palette },
  { id: "caja", nombre: "Caja y fiados", Icono: Wallet },
  { id: "impresora", nombre: "Impresora y escáner", Icono: Printer },
  { id: "seguridad", nombre: "Seguridad", Icono: ShieldCheck },
  { id: "datos", nombre: "Datos y respaldos", Icono: DatabaseBackup },
  { id: "licencia", nombre: "Licencia", Icono: BadgeCheck },
  { id: "acerca", nombre: "Acerca de", Icono: Info },
];

/** La última sección abierta se recuerda mientras la app está abierta (no se guarda en disco). */
let ultimaSeccion: IdSeccion = "negocio";

/** Para abrir Configuración en una sección (el aviso de la licencia lleva a Licencia). */
export function irASeccionConfig(id: IdSeccion) {
  ultimaSeccion = id;
}

const TEMAS: { id: Tema; nombre: string; Icono: LucideIcon }[] = [
  { id: "claro", nombre: "Claro", Icono: Sun },
  { id: "oscuro", nombre: "Oscuro", Icono: Moon },
  { id: "sistema", nombre: "Sistema", Icono: Monitor },
];

const MODOS_BARRA: Opcion<ModoBarra>[] = [
  { id: "auto", nombre: "Automática" },
  { id: "expandida", nombre: "Expandida" },
  { id: "colapsada", nombre: "Colapsada" },
];

export default function Configuracion({
  modoBarra,
  onModoBarra,
  onCambioImpresora,
  licencia,
  onCambioLicencia,
  onAbrirAsistente,
}: Props) {
  const [seccion, setSeccionEstado] = useState<IdSeccion>(ultimaSeccion);
  const [tema, setTema] = useTema();
  // El PIN se pide en el primer cambio y se recuerda mientras esta pantalla está abierta.
  const { conPin, olvidar } = useConPinRecordado();
  const cfg = useConfig(conPin);

  const elegir = (id: IdSeccion) => {
    ultimaSeccion = id;
    setSeccionEstado(id);
  };

  const contenido: Record<IdSeccion, ReactNode> = {
    negocio: <NegocioConfig conPin={conPin} />,
    apariencia: <Apariencia tema={tema} onTema={setTema} modoBarra={modoBarra} onModoBarra={onModoBarra} />,
    caja: <CajaYFiados cfg={cfg} />,
    impresora: (
      <ImpresoraConfig
        cfg={cfg}
        onCambioImpresora={onCambioImpresora}
        escaner={<EscanerConfig cfg={cfg} />}
      />
    ),
    seguridad: <SeguridadConfig conPin={conPin} onPinCambiado={olvidar} />,
    datos: <RespaldosConfig cfg={cfg} />,
    licencia: <LicenciaConfig estado={licencia} onCambio={onCambioLicencia} />,
    acerca: <AcercaDe />,
  };

  return (
    <div className="flex h-full flex-col px-6 pt-6">
      <header className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h1 className="text-[22px] font-[650]">Configuración</h1>
          <p className="mt-0.5 text-tenue">Ajustes del negocio, la caja y el equipo.</p>
        </div>
        <Boton icono={Wand2} onClick={onAbrirAsistente}>
          Asistente de configuración
        </Boton>
      </header>
      {cfg.error && (
        <p role="alert" className="mt-4 rounded-lg bg-peligro-fondo px-4 py-3 text-peligro">
          No se guardó el cambio: {cfg.error}
        </p>
      )}

      <div className="mt-5 flex min-h-0 flex-1 gap-6">
        <nav aria-label="Secciones de Configuración" className="sticky top-0 flex w-52 shrink-0 flex-col gap-1 self-start">
          {SECCIONES.map(({ id, nombre, Icono }) => {
            const activa = seccion === id;
            return (
              <Boton
                key={id}
                variante="terciario"
                bloque
                icono={Icono}
                aria-current={activa ? "page" : undefined}
                onClick={() => elegir(id)}
                className={activa ? "bg-seleccion font-semibold text-acento hover:bg-seleccion hover:text-acento" : "text-texto"}
              >
                {nombre}
              </Boton>
            );
          })}
        </nav>

        {/* El contenedor mide el ancho real de la sección: bajo 1100 px las tarjetas pasan a una columna. */}
        <div className="@container min-w-0 flex-1 overflow-y-auto pr-1 pb-6">
          <div className="max-w-[1280px]">{contenido[seccion]}</div>
        </div>
      </div>
    </div>
  );
}

function Apariencia({
  tema,
  onTema,
  modoBarra,
  onModoBarra,
}: {
  tema: Tema;
  onTema: (t: Tema) => void;
  modoBarra: ModoBarra;
  onModoBarra: (m: ModoBarra) => void;
}) {
  return (
    <Columnas
      izquierda={
        <Tarjeta titulo="Tema" ayuda="Sistema sigue el modo claro u oscuro de Windows.">
          <div role="radiogroup" aria-label="Tema" className="grid grid-cols-3 gap-3">
            {TEMAS.map(({ id, nombre, Icono }) => (
              <label
                key={id}
                className="relative flex cursor-pointer flex-col gap-2 rounded-panel border-2 border-linea p-2 hover:border-borde-control has-checked:border-acento has-checked:bg-seleccion has-focus-visible:outline-2 has-focus-visible:outline-offset-2 has-focus-visible:outline-acento"
              >
                <input
                  type="radio"
                  name="tema"
                  value={id}
                  checked={tema === id}
                  onChange={() => onTema(id)}
                  className="sr-only"
                />
                <Miniatura tema={id} />
                <span className="flex items-center gap-1.5 px-1 text-sm font-medium">
                  <Icono size={16} aria-hidden /> {nombre}
                </span>
              </label>
            ))}
          </div>
        </Tarjeta>
      }
      derecha={
        <Tarjeta titulo="Barra lateral">
          <Ajuste etiqueta="Modo" ayuda="Automática: se colapsa en la Caja y se expande en las demás secciones.">
            <Opciones nombre="barra" etiqueta="Barra lateral" opciones={MODOS_BARRA} valor={modoBarra} onCambiar={onModoBarra} />
          </Ajuste>
        </Tarjeta>
      }
    />
  );
}

/** Una ventana dibujada con divs, para elegir el tema sin leer su nombre. Los colores vienen de .muestra-*. */
function Miniatura({ tema }: { tema: Tema }) {
  return (
    <div aria-hidden className="relative h-[62px] overflow-hidden rounded-lg border border-linea">
      <Ventana clase="muestra-claro" />
      {tema === "oscuro" && <Ventana clase="muestra-oscuro" />}
      {tema === "sistema" && (
        <div className="absolute inset-0 [clip-path:inset(0_0_0_50%)]">
          <Ventana clase="muestra-oscuro" />
        </div>
      )}
    </div>
  );
}

function Ventana({ clase }: { clase: string }) {
  return (
    <div className={`muestra ${clase} absolute inset-0 flex bg-(--m-fondo)`}>
      <div className="w-1/4 border-r border-(--m-linea) bg-(--m-sup) p-1.5">
        <div className="h-1.5 rounded-sm bg-(--m-acento)" />
        <div className="mt-1 h-1.5 rounded-sm bg-(--m-linea)" />
        <div className="mt-1 h-1.5 rounded-sm bg-(--m-linea)" />
      </div>
      <div className="flex flex-1 flex-col gap-1.5 p-1.5">
        <div className="h-1.5 w-1/2 rounded-sm bg-(--m-texto) opacity-70" />
        <div className="flex-1 rounded-sm border border-(--m-linea) bg-(--m-sup)" />
        <div className="h-2 w-1/3 rounded-sm bg-(--m-acento)" />
      </div>
    </div>
  );
}

const SI_NO_DIAS: Opcion<string>[] = ["7", "15", "30", "60", "90"].map((d) => ({ id: d, nombre: `${d} días` }));

function CajaYFiados({ cfg }: { cfg: ConfigEditable }) {
  const { config, cambiar } = cfg;
  if (config === null) return <p className="text-tenue">Cargando…</p>;
  const dias = String(config.fiadoDiasAlerta);
  return (
    <Columnas
      izquierda={
        <Tarjeta titulo="Efectivo">
          <Ajuste
            etiqueta="Redondeo del efectivo"
            ayuda="Cobra en efectivo a la decena: 1 a 5 pesos hacia abajo, 6 a 9 hacia arriba. Tarjetas y transferencias no se redondean."
          >
            <Interruptor
              etiqueta="Redondeo del efectivo"
              marcado={config.redondeoEfectivo}
              onCambiar={(v) => void cambiar({ redondeoEfectivo: v })}
            />
          </Ajuste>
        </Tarjeta>
      }
      derecha={
        <Tarjeta titulo="Fiados">
          <Ajuste
            etiqueta="Resaltar deudas antiguas"
            ayuda="En Fiados se marca al cliente cuya venta pendiente más antigua tiene al menos estos días."
          >
            <Opciones
              nombre="dias-alerta"
              etiqueta="Días para resaltar una deuda"
              opciones={SI_NO_DIAS.some((o) => o.id === dias) ? SI_NO_DIAS : [...SI_NO_DIAS, { id: dias, nombre: `${dias} días` }]}
              valor={dias}
              onCambiar={(v) => void cambiar({ fiadoDiasAlerta: Number(v) })}
            />
          </Ajuste>
        </Tarjeta>
      }
    />
  );
}

function AcercaDe() {
  const [estado, setEstado] = useState<EstadoSistema | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.estadoSistema().then(setEstado).catch((e) => setError(String(e)));
  }, []);

  const datos: [string, string | number][] = estado
    ? [
        ["Versión", estado.version],
        ["Esquema de base de datos", `v${estado.versionEsquema}`],
        ["Arranque del backend", `${estado.msInicio} ms`],
        ["Productos", estado.totalProductos],
        ["Archivo de datos", estado.rutaBaseDatos],
      ]
    : [];
  return (
    <Columnas
      izquierda={
        <Tarjeta titulo="Acerca de" ayuda="Datos del sistema, para cuando pidas ayuda.">
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
      }
    />
  );
}
