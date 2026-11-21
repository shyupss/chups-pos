import { Monitor, Moon, Sun } from "lucide-react";
import { useEffect, useState } from "react";
import { api, type EstadoSistema } from "../api";
import { Bloque, Fila } from "../componentes/Ajustes";
import type { ModoBarra } from "../componentes/BarraLateral";
import Opciones, { type Opcion } from "../componentes/Opciones";
import NegocioConfig from "./configuracion/Negocio";
import { useConfig, type ConfigEditable } from "../hooks/useConfig";
import { useTema, type Tema } from "../hooks/useTema";

interface Props {
  modoBarra: ModoBarra;
  onModoBarra: (modo: ModoBarra) => void;
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

export default function Configuracion({ modoBarra, onModoBarra }: Props) {
  const [tema, setTema] = useTema();
  const cfg = useConfig();

  return (
    <div className="h-full overflow-auto p-6">
      <div className="max-w-3xl space-y-4">
        <h1 className="text-xl font-semibold">Configuración</h1>

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

        <Bloque titulo="Negocio">
          <NegocioConfig />
        </Bloque>
        <Bloque titulo="Impresora y escáner" fase={3}>
          Impresora térmica o PDF y lector de códigos de barras.
        </Bloque>
        <Bloque titulo="Seguridad" fase={4}>
          PIN de administrador para las acciones sensibles.
        </Bloque>
        <Bloque titulo="Datos y respaldos" fase={6}>
          Importar y exportar Excel, y respaldos de la base de datos.
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
  const { config, error, cambiar } = cfg;
  return (
    <Fila
      etiqueta="Redondeo del efectivo"
      ayuda="Cobra en efectivo a la decena: 1 a 5 pesos hacia abajo, 6 a 9 hacia arriba. Tarjetas y transferencias no se redondean."
    >
      {error ? (
        <span className="text-peligro">{error}</span>
      ) : config === null ? (
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
