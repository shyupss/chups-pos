import { Monitor, Moon, Sun } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { api, textoError, type EstadoSistema } from "../api";
import type { ModoBarra } from "../componentes/BarraLateral";
import Opciones, { type Opcion } from "../componentes/Opciones";
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
          <RedondeoEfectivo />
        </Bloque>

        <Bloque titulo="Negocio" fase={3}>
          Datos del negocio que aparecen en la boleta.
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

function Bloque({ titulo, fase, children }: { titulo: string; fase?: number; children: ReactNode }) {
  return (
    <section className="rounded-lg border border-linea bg-superficie p-5">
      <h2 className="font-semibold">{titulo}</h2>
      {fase === undefined ? (
        <div className="mt-3">{children}</div>
      ) : (
        <p className="mt-1 text-tenue">
          {children} Llega en la fase {fase}.
        </p>
      )}
    </section>
  );
}

function Fila({ etiqueta, ayuda, children }: { etiqueta: string; ayuda: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6">
      <div className="min-w-0">
        <div className="font-medium">{etiqueta}</div>
        <div className="text-xs text-tenue">{ayuda}</div>
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

/** Interruptor de la clave redondeo_efectivo de la tabla config. */
function RedondeoEfectivo() {
  const [activo, setActivo] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .configCaja()
      .then((c) => setActivo(c.redondeoEfectivo))
      .catch((e) => setError(textoError(e)));
  }, []);

  const cambiar = (valor: "si" | "no") => {
    const nuevo = valor === "si";
    setActivo(nuevo);
    api.cambiarRedondeoEfectivo(nuevo).catch((e) => {
      setActivo(!nuevo);
      setError(textoError(e));
    });
  };

  return (
    <Fila
      etiqueta="Redondeo del efectivo"
      ayuda="Cobra en efectivo a la decena: 1 a 5 pesos hacia abajo, 6 a 9 hacia arriba. Tarjetas y transferencias no se redondean."
    >
      {error ? (
        <span className="text-peligro">{error}</span>
      ) : activo === null ? (
        <span className="text-tenue">Cargando…</span>
      ) : (
        <Opciones
          nombre="redondeo"
          etiqueta="Redondeo del efectivo"
          opciones={SI_NO}
          valor={activo ? "si" : "no"}
          onCambiar={cambiar}
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
