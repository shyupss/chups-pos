import { Monitor, Moon, Sun } from "lucide-react";
import { useEffect, useState } from "react";
import { api, type EstadoSistema } from "../api";
import { Bloque, Fila } from "../componentes/Ajustes";
import type { ModoBarra } from "../componentes/BarraLateral";
import Opciones, { type Opcion } from "../componentes/Opciones";
import { useConPinRecordado } from "../componentes/PedirPin";
import DatosConfig from "./configuracion/Datos";
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
}

const TEMAS: Opcion<Tema>[] = [
  { id: "claro", nombre: "Claro", Icono: Sun },
  { id: "oscuro", nombre: "Oscuro", Icono: Moon },
  { id: "sistema", nombre: "Sistema", Icono: Monitor },
];

const MODOS_BARRA: Opcion<ModoBarra>[] = [
  { id: "auto", nombre: "Automática" },
  { id: "expandida", nombre: "Expandida" },
  { id: "colapsada", nombre: "Colapsada" },
];

const SI_NO: Opcion<"si" | "no">[] = [
  { id: "si", nombre: "Activado" },
  { id: "no", nombre: "Desactivado" },
];

export default function Configuracion({ modoBarra, onModoBarra, onCambioImpresora }: Props) {
  const [tema, setTema] = useTema();
  // El PIN se pide en el primer cambio y se recuerda mientras esta pantalla está abierta.
  const { conPin, olvidar } = useConPinRecordado();
  const cfg = useConfig(conPin);

  return (
    <div className="h-full overflow-auto p-6">
      <div className="max-w-3xl space-y-4">
        <h1 className="text-xl font-semibold">Configuración</h1>
        {cfg.error && (
          <p role="alert" className="sticky top-0 z-10 rounded-lg bg-peligro/10 px-4 py-3 text-peligro">
            No se guardó el cambio: {cfg.error}
          </p>
        )}

        <Bloque titulo="Apariencia">
          <div className="space-y-4">
            <Fila etiqueta="Tema" ayuda="Sistema sigue el modo claro u oscuro de Windows.">
              <Opciones nombre="tema" etiqueta="Tema" opciones={TEMAS} valor={tema} onCambiar={setTema} />
            </Fila>
            <Fila etiqueta="Barra lateral" ayuda="Automática: se colapsa en la Caja y se expande en las demás secciones.">
              <Opciones
                nombre="barra"
                etiqueta="Barra lateral"
                opciones={MODOS_BARRA}
                valor={modoBarra}
                onCambiar={onModoBarra}
              />
            </Fila>
          </div>
        </Bloque>

        <Bloque titulo="Caja">
          <RedondeoEfectivo cfg={cfg} />
        </Bloque>

        <Bloque titulo="Fiados">
          <DiasAlerta cfg={cfg} />
        </Bloque>

        <Bloque titulo="Negocio">
          <NegocioConfig conPin={conPin} />
        </Bloque>
        <Bloque titulo="Impresora y escáner">
          <ImpresoraConfig cfg={cfg} onCambioImpresora={onCambioImpresora} />
          <EscanerConfig cfg={cfg} />
        </Bloque>
        <Bloque titulo="Seguridad">
          <SeguridadConfig conPin={conPin} onPinCambiado={olvidar} />
        </Bloque>
        <Bloque titulo="Datos y respaldos">
          <DatosConfig
            respaldos={
              <div className="space-y-3 border-t border-linea pt-5">
                <h3 className="font-medium">Respaldos</h3>
                <RespaldosConfig cfg={cfg} />
              </div>
            }
          />
        </Bloque>
        <Bloque titulo="Licencia" fase={7}>
          Activación de la licencia.
        </Bloque>

        <Bloque titulo="Acerca de">
          <AcercaDe />
        </Bloque>
      </div>
    </div>
  );
}

/** Interruptor de la clave redondeo_efectivo de la tabla config. */
function RedondeoEfectivo({ cfg }: { cfg: ConfigEditable }) {
  const { config, cambiar } = cfg;
  return (
    <Fila
      etiqueta="Redondeo del efectivo"
      ayuda="Cobra en efectivo a la decena: 1 a 5 pesos hacia abajo, 6 a 9 hacia arriba. Tarjetas y transferencias no se redondean."
    >
      {config === null ? (
        <span className="text-tenue">Cargando…</span>
      ) : (
        <Opciones
          nombre="redondeo"
          etiqueta="Redondeo del efectivo"
          opciones={SI_NO}
          valor={config.redondeoEfectivo ? "si" : "no"}
          onCambiar={(v) => void cambiar({ redondeoEfectivo: v === "si" })}
        />
      )}
    </Fila>
  );
}

const DIAS_ALERTA: Opcion<string>[] = ["7", "15", "30", "60", "90"].map((d) => ({ id: d, nombre: `${d} días` }));

/** Clave fiado_dias_alerta: desde cuántos días de deuda se resalta al cliente en Fiados. */
function DiasAlerta({ cfg }: { cfg: ConfigEditable }) {
  const { config, cambiar } = cfg;
  return (
    <Fila
      etiqueta="Resaltar deudas antiguas"
      ayuda="En Fiados se marca al cliente cuya venta pendiente más antigua tiene al menos estos días."
    >
      {config === null ? (
        <span className="text-tenue">Cargando…</span>
      ) : (
        <Opciones
          nombre="dias-alerta"
          etiqueta="Días para resaltar una deuda"
          opciones={DIAS_ALERTA.some((o) => o.id === String(config.fiadoDiasAlerta)) ? DIAS_ALERTA : [...DIAS_ALERTA, { id: String(config.fiadoDiasAlerta), nombre: `${config.fiadoDiasAlerta} días` }]}
          valor={String(config.fiadoDiasAlerta)}
          onCambiar={(v) => void cambiar({ fiadoDiasAlerta: Number(v) })}
        />
      )}
    </Fila>
  );
}

function AcercaDe() {
  const [estado, setEstado] = useState<EstadoSistema | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.estadoSistema().then(setEstado).catch((e) => setError(String(e)));
  }, []);

  if (error) return <p className="text-peligro">No se pudo leer el estado del sistema: {error}</p>;
  if (!estado) return <p className="text-tenue">Cargando…</p>;

  const datos: [string, string | number][] = [
    ["Versión", estado.version],
    ["Esquema de base de datos", `v${estado.versionEsquema}`],
    ["Arranque del backend", `${estado.msInicio} ms`],
    ["Productos", estado.totalProductos],
    ["Archivo de datos", estado.rutaBaseDatos],
  ];
  return (
    <dl className="grid grid-cols-[12rem_1fr] gap-x-4 gap-y-2 tabular-nums">
      {datos.map(([etiqueta, valor]) => (
        <div key={etiqueta} className="contents">
          <dt className="text-tenue">{etiqueta}</dt>
          <dd className="min-w-0 break-all select-text">{valor}</dd>
        </div>
      ))}
    </dl>
  );
}
