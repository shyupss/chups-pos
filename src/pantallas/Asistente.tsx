import { Check, FileSpreadsheet, PackagePlus, ShoppingCart } from "lucide-react";
import { useEffect, useState } from "react";
import { api, textoError, type EstadoLicencia, type EstadoSeguridad } from "../api";
import { Tarjeta } from "../componentes/Ajustes";
import Boton from "../componentes/Boton";
import DialogoImportar from "../componentes/DialogoImportar";
import { useConPin } from "../componentes/PedirPin";
import { avisarCambioStock } from "../eventos";
import { useConfig } from "../hooks/useConfig";
import { pedirSalida } from "../salida";
import { PruebaEscaner } from "./configuracion/Escaner";
import ImpresoraConfig from "./configuracion/Impresora";
import LicenciaConfig from "./configuracion/Licencia";
import NegocioConfig from "./configuracion/Negocio";
import { CodigoRecuperacion, CrearPin } from "./configuracion/Seguridad";

/** Adónde va la app al terminar el asistente. */
export type DestinoAsistente = "caja" | "productos" | "producto-nuevo";

interface Props {
  /** Se abrió desde Configuración (no es el primer arranque): se puede salir en cualquier momento. */
  reabierto: boolean;
  /** Cambió la impresora o se probó: la barra lateral vuelve a leer su estado. */
  onCambioImpresora: () => void;
  onTerminar: (destino: DestinoAsistente) => void;
}

const PASOS = ["Negocio", "Impresora", "Escáner", "PIN", "Licencia"] as const;

/**
 * Asistente de primer arranque, en 5 pasos con "Omitir" en cada uno menos el primero (sin el nombre
 * del negocio no hay boleta). Cada paso reutiliza lo de Configuración. Al final ofrece importar
 * productos desde Excel o crear el primero.
 */
export default function Asistente({ reabierto, onCambioImpresora, onTerminar }: Props) {
  const conPin = useConPin();
  const cfg = useConfig(conPin);
  const [paso, setPaso] = useState(0);
  const [final, setFinal] = useState(false);
  const [negocioListo, setNegocioListo] = useState(false);
  const [seguridad, setSeguridad] = useState<EstadoSeguridad | null>(null);
  const [licencia, setLicencia] = useState<EstadoLicencia | null>(null);
  const [codigoNuevo, setCodigoNuevo] = useState<string | null>(null);
  const [importando, setImportando] = useState(false);
  const [importado, setImportado] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const leerSeguridad = () => {
    api
      .estadoSeguridad()
      .then(setSeguridad)
      .catch(() => {});
  };
  useEffect(() => {
    api
      .obtenerNegocio()
      .then((n) => setNegocioListo(n.nombre.trim() !== ""))
      .catch(() => {});
    leerSeguridad();
    api
      .estadoLicencia()
      .then(setLicencia)
      .catch(() => {});
  }, []);

  // Qué paso ya está hecho: habilita "Siguiente" (omitirlo se puede igual, salvo el primero).
  const hecho = [
    negocioListo,
    cfg.config?.impresora != null,
    true,
    seguridad?.tienePin === true,
    licencia?.modo === "activa",
  ];

  // Moverse entre pasos o salir pregunta antes si quedaron cambios sin guardar en Negocio.
  const irAPaso = (n: number) => pedirSalida(() => setPaso(n));
  const siguiente = () => pedirSalida(() => (paso === PASOS.length - 1 ? setFinal(true) : setPaso(paso + 1)));

  const terminar = async (destino: DestinoAsistente) => {
    try {
      await api.terminarAsistente();
      onTerminar(destino);
    } catch (e) {
      setError(textoError(e));
    }
  };

  const contenido = [
    <NegocioConfig key="negocio" conPin={conPin} onGuardado={(n) => setNegocioListo(n.nombre.trim() !== "")} />,
    <ImpresoraConfig key="impresora" cfg={cfg} onCambioImpresora={onCambioImpresora} soloImpresora />,
    <Tarjeta
      key="escaner"
      titulo="Escáner de códigos"
      ayuda="Escanea cualquier producto para comprobar que el lector funciona. Si todavía no tienes escáner, omite este paso."
    >
      <PruebaEscaner />
    </Tarjeta>,
    <Tarjeta
      key="pin"
      titulo="PIN de administrador"
      ayuda="Protege anular ventas, devoluciones, retiros de efectivo y cambios en la configuración."
    >
      {seguridad?.tienePin ? (
        <p className="w-fit rounded-lg bg-exito-fondo px-3 py-2 font-medium text-exito">PIN de administrador activo</p>
      ) : (
        <CrearPin onCreado={(codigo) => setCodigoNuevo(codigo)} />
      )}
    </Tarjeta>,
    <LicenciaConfig key="licencia" estado={licencia} onCambio={setLicencia} />,
  ];

  return (
    <div className="flex h-full flex-col overflow-y-auto">
      <div className="mx-auto flex w-full max-w-6xl flex-1 flex-col gap-6 p-6">
        <header className="flex flex-wrap items-center gap-4">
          <div className="min-w-0 flex-1">
            <h1 className="text-[22px] font-[650]">{final ? "Todo listo" : "Bienvenido a CHUPS POS"}</h1>
            <p className="mt-0.5 text-tenue">
              {final
                ? "Ya puedes empezar a vender. Lo que omitiste lo encuentras en Configuración."
                : "Configuremos tu negocio en unos minutos."}
            </p>
          </div>
          {reabierto && (
            <Boton variante="terciario" onClick={() => pedirSalida(() => void terminar("caja"))}>
              Salir del asistente
            </Boton>
          )}
        </header>

        {!final && (
          <ol aria-label="Pasos del asistente" className="flex flex-wrap gap-2">
            {PASOS.map((nombre, i) => (
              <li
                key={nombre}
                aria-current={i === paso ? "step" : undefined}
                className={`flex h-8 items-center gap-2 rounded-lg px-3 text-[13px] ${
                  i === paso ? "bg-seleccion font-semibold text-acento" : "bg-suave text-tenue"
                }`}
              >
                <span className="flex size-5 items-center justify-center rounded-full border border-current text-xs">
                  {hecho[i] && i < paso ? <Check size={12} aria-hidden /> : i + 1}
                </span>
                {nombre}
              </li>
            ))}
          </ol>
        )}

        {error && (
          <p role="alert" className="rounded-lg bg-peligro-fondo px-4 py-3 text-peligro">
            {error}
          </p>
        )}
        {cfg.error && (
          <p role="alert" className="rounded-lg bg-peligro-fondo px-4 py-3 text-peligro">
            No se guardó el cambio: {cfg.error}
          </p>
        )}

        {final ? (
          <Tarjeta titulo="¿Cómo quieres empezar?" ayuda="Elige una opción; siempre puedes cambiar de idea después.">
            <div className="flex flex-col items-start gap-3">
              <Boton variante="primario" icono={FileSpreadsheet} onClick={() => setImportando(true)}>
                Importar mis productos desde Excel
              </Boton>
              <Boton icono={PackagePlus} onClick={() => void terminar("producto-nuevo")}>
                Crear el primer producto
              </Boton>
              <Boton variante="terciario" icono={ShoppingCart} onClick={() => void terminar("caja")}>
                Ir a la Caja
              </Boton>
            </div>
          </Tarjeta>
        ) : (
          // El contenedor mide el ancho para que Negocio use dos columnas en pantallas grandes.
          <div className="@container min-h-0 flex-1">{contenido[paso]}</div>
        )}

        {!final && (
          <footer className="sticky bottom-0 -mx-6 flex flex-wrap items-center gap-2 border-t border-linea bg-fondo px-6 py-3">
            <Boton variante="terciario" disabled={paso === 0} onClick={() => irAPaso(paso - 1)}>
              Atrás
            </Boton>
            <span className="flex-1 text-[13px] text-tenue">
              Paso {paso + 1} de {PASOS.length}
            </span>
            {paso > 0 && (
              <Boton onClick={siguiente}>
                {paso === PASOS.length - 1 ? "Seguir en prueba" : "Omitir"}
              </Boton>
            )}
            <Boton
              variante="primario"
              disabled={!hecho[paso]}
              onClick={siguiente}
            >
              {paso === PASOS.length - 1 ? "Terminar" : "Siguiente"}
            </Boton>
          </footer>
        )}
      </div>

      {codigoNuevo && (
        <CodigoRecuperacion
          codigo={codigoNuevo}
          onCerrar={() => {
            setCodigoNuevo(null);
            leerSeguridad();
          }}
        />
      )}
      {importando && (
        <DialogoImportar
          tipo="productos"
          onCerrar={() => {
            setImportando(false);
            // Si se importó algo, se ve el resultado y al cerrarlo se entra a la Caja.
            if (importado) void terminar("caja");
          }}
          onImportado={() => {
            avisarCambioStock();
            setImportado(true);
          }}
        />
      )}
    </div>
  );
}
