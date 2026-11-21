import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
  type RefObject,
} from "react";
import { errorPin, type ErrorPin } from "../api";
import Dialogo from "./Dialogo";

/** Se canceló el diálogo del PIN: la acción no se hizo y no hay error que mostrar. */
export class PinCancelado extends Error {
  constructor() {
    super("Se canceló el PIN");
  }
}

export const esCancelado = (e: unknown) => e instanceof PinCancelado;

/** Llama al comando protegido con el PIN escrito (o null en el primer intento). */
type Ejecutar<T> = (pin: string | null) => Promise<T>;

export interface OpcionesPin {
  /** Lo que se va a hacer, para el diálogo: "Anular la venta N° 000123". */
  accion?: string;
  /** Guarda el PIN aceptado para las llamadas siguientes (Configuración, mientras está abierta). */
  recordar?: RefObject<string | null>;
}

/**
 * Ejecuta un comando protegido. Primero lo intenta sin PIN (o con el recordado): si la acción no
 * está protegida, listo. Si Rust responde que falta el PIN, abre el diálogo y reintenta con lo que
 * se escriba hasta que Rust lo acepte o se cancele (rechaza con PinCancelado). Los errores que no
 * son del PIN se rechazan tal cual. La vista nunca decide si el PIN es correcto.
 */
export type ConPin = <T>(ejecutar: Ejecutar<T>, opciones?: OpcionesPin) => Promise<T>;

interface Pedido {
  accion?: string;
  inicial: ErrorPin | null;
  ejecutar: Ejecutar<unknown>;
  resolver: (valor: unknown) => void;
  rechazar: (e: unknown) => void;
  recordar?: RefObject<string | null>;
}

const Contexto = createContext<ConPin | null>(null);

/** Va una vez arriba de la app: muestra el diálogo del PIN cuando un comando lo pide. */
export function ProveedorPin({ children }: { children: ReactNode }) {
  const [pedido, setPedido] = useState<Pedido | null>(null);
  const turno = useRef(0);

  const conPin = useCallback(async <T,>(ejecutar: Ejecutar<T>, opciones: OpcionesPin = {}): Promise<T> => {
    const { accion, recordar } = opciones;
    try {
      return await ejecutar(recordar?.current ?? null);
    } catch (e) {
      const fallo = errorPin(e);
      if (!fallo) throw e;
      if (recordar) recordar.current = null;
      turno.current += 1;
      return new Promise<T>((resolver, rechazar) => {
        setPedido({
          accion,
          inicial: fallo.pin === "requerido" ? null : fallo,
          ejecutar,
          resolver: resolver as (valor: unknown) => void,
          rechazar,
          recordar,
        });
      });
    }
  }, []);

  const confirmar = async (p: Pedido, pin: string): Promise<ErrorPin | null> => {
    try {
      const valor = await p.ejecutar(pin);
      if (p.recordar) p.recordar.current = pin;
      setPedido(null);
      p.resolver(valor);
    } catch (e) {
      const fallo = errorPin(e);
      if (fallo) return fallo;
      // El PIN pasó, pero la acción falló por otra cosa: el error lo muestra quien la pidió.
      setPedido(null);
      p.rechazar(e);
    }
    return null;
  };

  return (
    <Contexto.Provider value={conPin}>
      {children}
      {pedido && (
        <PedirPin
          key={turno.current}
          accion={pedido.accion}
          inicial={pedido.inicial}
          onConfirmar={(pin) => confirmar(pedido, pin)}
          onCancelar={() => {
            setPedido(null);
            pedido.rechazar(new PinCancelado());
          }}
        />
      )}
    </Contexto.Provider>
  );
}

export function useConPin(): ConPin {
  const conPin = useContext(Contexto);
  if (!conPin) throw new Error("Falta <ProveedorPin> arriba de la app");
  return conPin;
}

/**
 * Como useConPin, pero recuerda el PIN aceptado mientras el componente está montado: en
 * Configuración se pide una vez y no en cada cambio. Rust lo vuelve a verificar en cada llamada.
 * `olvidar` se llama cuando el PIN cambia o se quita.
 */
export function useConPinRecordado(): { conPin: ConPin; olvidar: () => void } {
  const conPin = useConPin();
  const recordado = useRef<string | null>(null);
  return useMemo(
    () => ({
      conPin: <T,>(ejecutar: Ejecutar<T>, opciones?: OpcionesPin) =>
        conPin(ejecutar, { ...opciones, recordar: recordado }),
      olvidar: () => {
        recordado.current = null;
      },
    }),
    [conPin],
  );
}

/** Solo dígitos, hasta 6. */
const digitos = (texto: string) => texto.replace(/\D/g, "").slice(0, 6);

interface Props {
  accion?: string;
  /** Error con que se abre (por ejemplo, el PIN recordado ya no sirve o está bloqueado). */
  inicial: ErrorPin | null;
  /** Ejecuta la acción con el PIN. Devuelve el error del PIN para mostrarlo, o null si terminó. */
  onConfirmar: (pin: string) => Promise<ErrorPin | null>;
  onCancelar: () => void;
}

/** Diálogo del PIN de administrador: campo numérico enmascarado, Enter confirma y Esc cancela. */
export default function PedirPin({ accion, inicial, onConfirmar, onCancelar }: Props) {
  const [pin, setPin] = useState("");
  const [error, setError] = useState<ErrorPin | null>(inicial);
  const [enviando, setEnviando] = useState(false);
  const [hasta, setHasta] = useState(() => (inicial?.segundos ? Date.now() + inicial.segundos * 1000 : 0));
  const [ahora, setAhora] = useState(() => Date.now());
  const campo = useRef<HTMLInputElement>(null);

  const faltan = Math.max(0, Math.ceil((hasta - ahora) / 1000));
  const bloqueado = faltan > 0;

  // Cuenta regresiva del bloqueo; al terminar, el campo vuelve a quedar listo.
  useEffect(() => {
    if (!bloqueado) return;
    const t = setInterval(() => setAhora(Date.now()), 250);
    return () => clearInterval(t);
  }, [bloqueado]);
  useEffect(() => {
    if (!bloqueado) campo.current?.focus();
  }, [bloqueado]);

  const confirmar = async () => {
    if (enviando || bloqueado || pin.length < 4) return;
    setEnviando(true);
    const fallo = await onConfirmar(pin);
    if (!fallo) return; // terminó: el proveedor cierra el diálogo
    setError(fallo);
    setPin("");
    setEnviando(false);
    if (fallo.segundos) {
      setHasta(Date.now() + fallo.segundos * 1000);
      setAhora(Date.now());
    }
    campo.current?.focus();
  };

  const mensaje = bloqueado
    ? `Demasiados intentos fallidos: espera ${faltan} ${faltan === 1 ? "segundo" : "segundos"} para volver a probar`
    : error?.pin === "bloqueado"
      ? "Ya puedes volver a probar"
      : error?.mensaje;

  return (
    <Dialogo titulo="PIN de administrador" onCerrar={onCancelar}>
      <p className="mt-1 text-tenue">{accion ? `Para ${accion.charAt(0).toLowerCase()}${accion.slice(1)}.` : "Esta acción pide el PIN."}</p>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void confirmar();
        }}
      >
        <input
          ref={campo}
          autoFocus
          value={pin}
          onChange={(e) => setPin(digitos(e.target.value))}
          disabled={bloqueado}
          inputMode="numeric"
          autoComplete="off"
          spellCheck={false}
          aria-label="PIN de administrador"
          aria-invalid={error !== null && error.pin !== "bloqueado"}
          aria-describedby="mensaje-pin"
          placeholder="4 a 6 dígitos"
          className="mt-4 h-13 w-full rounded-lg border border-borde-control bg-superficie px-4 text-center text-2xl tracking-[0.5em] tabular-nums [-webkit-text-security:disc] placeholder:tracking-normal placeholder:[-webkit-text-security:none] focus:border-acento disabled:opacity-60"
        />
        <p
          id="mensaje-pin"
          role={mensaje ? "alert" : undefined}
          className={`mt-3 min-h-10 rounded-lg px-3 py-2 ${
            !mensaje
              ? "text-xs text-tenue"
              : bloqueado
                ? "bg-aviso-fondo text-aviso"
                : error?.pin === "bloqueado"
                  ? "bg-exito-fondo text-exito"
                  : "bg-peligro/10 text-peligro"
          }`}
        >
          {mensaje ?? "¿Lo olvidaste? Ponle uno nuevo con el código de recuperación en Configuración > Seguridad."}
        </p>
        <div className="mt-4 flex gap-2">
          <button type="button" onClick={onCancelar} className="h-11 flex-1 rounded-lg text-tenue hover:bg-fondo hover:text-texto">
            Cancelar <span className="text-xs opacity-60">Esc</span>
          </button>
          <button
            type="submit"
            disabled={enviando || bloqueado || pin.length < 4}
            className="h-11 flex-[2] rounded-lg bg-acento font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
          >
            {enviando ? "Verificando…" : "Confirmar"} <span className="text-xs font-normal opacity-70">Enter</span>
          </button>
        </div>
      </form>
    </Dialogo>
  );
}
