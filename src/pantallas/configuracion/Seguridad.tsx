import Interruptor from "../../componentes/Interruptor";
import Boton from "../../componentes/Boton";
import { KeyRound, ShieldCheck } from "lucide-react";
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { api, textoError, type AccionesProtegidas, type EstadoSeguridad } from "../../api";
import { Ajuste, Columnas, Tarjeta } from "../../componentes/Ajustes";
import Dialogo from "../../componentes/Dialogo";

import { esCancelado, type ConPin } from "../../componentes/PedirPin";


const ACCIONES: { clave: keyof AccionesProtegidas; nombre: string; ayuda: string }[] = [
  { clave: "anularVenta", nombre: "Anular una venta", ayuda: "Repone el stock y devuelve el dinero de toda la venta." },
  { clave: "devolucion", nombre: "Devolver productos", ayuda: "Devoluciones parciales desde el detalle de una venta." },
  { clave: "retiroEfectivo", nombre: "Retirar efectivo", ayuda: "Sacar dinero de la caja durante el turno." },
  { clave: "cerrarCaja", nombre: "Cerrar la caja", ayuda: "El cierre del turno con el conteo del efectivo." },
  {
    clave: "ajustarInventario",
    nombre: "Ajustar inventario",
    ayuda: "Ajustes por conteo, mermas y anular compras.",
  },
  {
    clave: "fiarSobreLimite",
    nombre: "Fiar sobre el límite",
    ayuda: "Vender fiado aunque la deuda pase el límite de crédito del cliente.",
  },
  {
    clave: "importarDatos",
    nombre: "Importar datos",
    ayuda: "Crear o actualizar productos y clientes desde una planilla de Excel.",
  },
  {
    clave: "cambiarConfiguracion",
    nombre: "Cambiar la configuración",
    ayuda: "Todo lo de esta pantalla menos la apariencia. El PIN se pide una vez mientras estás aquí.",
  },
];

const claseCampo =
  "h-11 w-full rounded-lg border border-borde-control bg-superficie px-3 text-center text-lg tracking-[0.4em] tabular-nums [-webkit-text-security:disc] focus:border-acento";


const digitos = (texto: string) => texto.replace(/\D/g, "").slice(0, 6);

type Dialogos = { tipo: "cambiar" } | { tipo: "recuperar" } | { tipo: "codigo"; codigo: string } | null;

interface Props {
  conPin: ConPin;
  /** El PIN cambió o se quitó: Configuración olvida el que tenía recordado. */
  onPinCambiado: () => void;
}

/** Configuración > Seguridad: crear, cambiar y quitar el PIN, y qué acciones lo piden. */
export default function SeguridadConfig({ conPin, onPinCambiado }: Props) {
  const [estado, setEstado] = useState<EstadoSeguridad | null>(null);
  const [mensaje, setMensaje] = useState<{ error: boolean; texto: string } | null>(null);
  const [dialogo, setDialogo] = useState<Dialogos>(null);
  const [ocupado, setOcupado] = useState(false);

  const leer = useCallback(() => {
    api
      .estadoSeguridad()
      .then(setEstado)
      .catch((e) => setMensaje({ error: true, texto: textoError(e) }));
  }, []);
  useEffect(leer, [leer]);

  const cambiarProtegida = async (clave: keyof AccionesProtegidas, valor: boolean) => {
    if (!estado || ocupado) return;
    const nuevas = { ...estado.protegidas, [clave]: valor };
    setOcupado(true);
    setMensaje(null);
    try {
      setEstado(await conPin((pin) => api.guardarProtegidas(nuevas, pin), { accion: "Cambiar las acciones protegidas" }));
    } catch (e) {
      if (!esCancelado(e)) setMensaje({ error: true, texto: textoError(e) });
    } finally {
      setOcupado(false);
    }
  };

  const quitar = async () => {
    setMensaje(null);
    try {
      await conPin((pin) => api.quitarPin(pin), { accion: "Quitar el PIN" });
      onPinCambiado();
      setMensaje({ error: false, texto: "PIN quitado: ninguna acción lo pide." });
      leer();
    } catch (e) {
      if (!esCancelado(e)) setMensaje({ error: true, texto: textoError(e) });
    }
  };

  if (!estado) return mensaje ? <p className="text-peligro">{mensaje.texto}</p> : <p className="text-tenue">Cargando…</p>;

  return (
    <>
      <Columnas
        izquierda={
          <>
            <Tarjeta titulo="PIN de administrador" ayuda="Protege las acciones sensibles con un PIN de 4 a 6 dígitos.">
              {estado.tienePin ? (
                <>
                  <span className="flex w-fit items-center gap-2 rounded-lg bg-exito-fondo px-3 py-2 font-medium text-exito">
                    <ShieldCheck size={16} aria-hidden /> PIN activo
                  </span>
                  <div className="flex flex-wrap gap-2">
                    <Boton onClick={() => setDialogo({ tipo: "cambiar" })}>Cambiar PIN</Boton>
                    <Boton onClick={() => setDialogo({ tipo: "recuperar" })}>Olvidé el PIN</Boton>
                    <Boton variante="terciario" onClick={() => void quitar()}>
                      Quitar PIN
                    </Boton>
                  </div>
                </>
              ) : (
                <CrearPin
                  onCreado={(codigo) => {
                    setDialogo({ tipo: "codigo", codigo });
                    setMensaje(null);
                    leer();
                  }}
                />
              )}
              {mensaje && (
                <p role="status" className={mensaje.error ? "text-peligro" : "text-exito"}>
                  {mensaje.texto}
                </p>
              )}
            </Tarjeta>
            <Tarjeta titulo="Código de recuperación" ayuda="Para poner un PIN nuevo si olvidas el actual.">
              <p className="text-tenue">
                Al crear el PIN se muestra una sola vez un código de 10 caracteres. Anótalo y guárdalo fuera del
                computador: con él, «Olvidé el PIN» pone uno nuevo y entrega un código nuevo (el usado deja de servir).
              </p>
              <p className="rounded-lg bg-info-fondo px-3 py-2 text-[13px] text-info">
                Si se pierden el PIN y el código, no hay otra forma de recuperarlo desde la app.
              </p>
            </Tarjeta>
          </>
        }
        derecha={
          <Tarjeta titulo="Acciones que piden el PIN" ayuda="Cambiar esta lista siempre lo pide.">
            {estado.tienePin ? (
              <>
                {ACCIONES.map((a) => (
                  <Ajuste key={a.clave} etiqueta={a.nombre} ayuda={a.ayuda}>
                    <Interruptor
                      etiqueta={`Pide PIN: ${a.nombre}`}
                      marcado={estado.protegidas[a.clave]}
                      desactivado={ocupado}
                      onCambiar={(v) => void cambiarProtegida(a.clave, v)}
                    />
                  </Ajuste>
                ))}
                <Ajuste etiqueta="Restaurar un respaldo" ayuda="Reemplaza toda la base de datos: siempre pide el PIN.">
                  <span className="text-tenue">Siempre</span>
                </Ajuste>
              </>
            ) : (
              <p className="text-tenue">Sin un PIN creado, ninguna acción lo pide. Crea uno para elegir cuáles proteger.</p>
            )}
          </Tarjeta>
        }
      />

      {dialogo?.tipo === "codigo" && <CodigoRecuperacion codigo={dialogo.codigo} onCerrar={() => setDialogo(null)} />}
      {dialogo?.tipo === "cambiar" && (
        <CambiarPin
          onCancelar={() => setDialogo(null)}
          onCambiado={() => {
            onPinCambiado();
            setDialogo(null);
            setMensaje({ error: false, texto: "PIN cambiado." });
          }}
        />
      )}
      {dialogo?.tipo === "recuperar" && (
        <RecuperarPin
          onCancelar={() => setDialogo(null)}
          onRecuperado={(codigo) => {
            onPinCambiado();
            setDialogo({ tipo: "codigo", codigo });
            setMensaje({ error: false, texto: "PIN nuevo guardado." });
          }}
        />
      )}
    </>
  );
}

function CampoPin({
  etiqueta,
  valor,
  onCambiar,
  autoFocus,
}: {
  etiqueta: string;
  valor: string;
  onCambiar: (v: string) => void;
  autoFocus?: boolean;
}) {
  return (
    <label className="block">
      <span className="text-sm text-tenue">{etiqueta}</span>
      <input
        autoFocus={autoFocus}
        value={valor}
        onChange={(e) => onCambiar(digitos(e.target.value))}
        inputMode="numeric"
        autoComplete="off"
        spellCheck={false}
        className={`mt-1 ${claseCampo}`}
      />
    </label>
  );
}

/** Revisa que el PIN nuevo esté escrito igual dos veces (el formato lo valida Rust). */
function useNuevoPin() {
  const [nuevo, setNuevo] = useState("");
  const [repetido, setRepetido] = useState("");
  const distinto = repetido.length >= nuevo.length && repetido !== nuevo && repetido !== "";
  const listo = nuevo.length >= 4 && nuevo === repetido;
  return { nuevo, setNuevo, repetido, setRepetido, distinto, listo };
}

function MensajeError({ texto }: { texto: string | null }) {
  return texto ? (
    <p role="alert" className="mt-3 rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">
      {texto}
    </p>
  ) : null;
}

export function CrearPin({ onCreado }: { onCreado: (codigo: string) => void }) {
  const p = useNuevoPin();
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);
  return (
    <form
      onSubmit={async (e) => {
        e.preventDefault();
        if (!p.listo || enviando) return;
        setEnviando(true);
        setError(null);
        try {
          onCreado(await api.crearPin(p.nuevo));
        } catch (e) {
          setError(textoError(e));
        } finally {
          setEnviando(false);
        }
      }}
    >
      <p className="text-tenue">
        Un PIN de 4 a 6 dígitos protege las acciones sensibles: anular ventas, devoluciones, retiros de efectivo y
        cambios en la configuración. Sin PIN, ninguna acción lo pide.
      </p>
      <div className="mt-3 grid max-w-md grid-cols-2 gap-3">
        <CampoPin etiqueta="PIN nuevo" valor={p.nuevo} onCambiar={p.setNuevo} />
        <CampoPin etiqueta="Repite el PIN" valor={p.repetido} onCambiar={p.setRepetido} />
      </div>
      {p.distinto && <p className="mt-2 text-xs text-peligro">Los dos PIN no coinciden.</p>}
      <MensajeError texto={error} />
      <Boton type="submit" variante="primario" icono={KeyRound} disabled={!p.listo || enviando} className="mt-3">
        Crear PIN
      </Boton>
    </form>
  );
}

function PieDialogo({ onCancelar, listo, enviando, texto }: { onCancelar: () => void; listo: boolean; enviando: boolean; texto: string }) {
  return (
    <div className="mt-5 flex gap-2">
      <Boton variante="terciario" atajo="Esc" onClick={onCancelar} className="flex-1">
        Cancelar
      </Boton>
      <Boton type="submit" variante="primario" atajo="Enter" disabled={!listo || enviando} className="flex-[2]">
        {enviando ? "Guardando…" : texto}
      </Boton>
    </div>
  );
}

function FormularioDialogo({
  titulo,
  onCancelar,
  onEnviar,
  children,
}: {
  titulo: string;
  onCancelar: () => void;
  onEnviar: () => void;
  children: ReactNode;
}) {
  return (
    <Dialogo titulo={titulo} onCerrar={onCancelar}>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          onEnviar();
        }}
        className="mt-4 space-y-3"
      >
        {children}
      </form>
    </Dialogo>
  );
}

function CambiarPin({ onCancelar, onCambiado }: { onCancelar: () => void; onCambiado: () => void }) {
  const [actual, setActual] = useState("");
  const p = useNuevoPin();
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);
  const listo = actual.length >= 4 && p.listo;
  const enviar = async () => {
    if (!listo || enviando) return;
    setEnviando(true);
    setError(null);
    try {
      await api.cambiarPin(actual, p.nuevo);
      onCambiado();
    } catch (e) {
      setError(textoError(e));
      setActual("");
      setEnviando(false);
    }
  };
  return (
    <FormularioDialogo titulo="Cambiar PIN" onCancelar={onCancelar} onEnviar={() => void enviar()}>
      <CampoPin etiqueta="PIN actual" valor={actual} onCambiar={setActual} autoFocus />
      <CampoPin etiqueta="PIN nuevo (4 a 6 dígitos)" valor={p.nuevo} onCambiar={p.setNuevo} />
      <CampoPin etiqueta="Repite el PIN nuevo" valor={p.repetido} onCambiar={p.setRepetido} />
      {p.distinto && <p className="text-xs text-peligro">Los dos PIN nuevos no coinciden.</p>}
      <p className="text-xs text-tenue">El código de recuperación sigue siendo el mismo.</p>
      <MensajeError texto={error} />
      <PieDialogo onCancelar={onCancelar} listo={listo} enviando={enviando} texto="Cambiar PIN" />
    </FormularioDialogo>
  );
}

function RecuperarPin({ onCancelar, onRecuperado }: { onCancelar: () => void; onRecuperado: (codigo: string) => void }) {
  const [codigo, setCodigo] = useState("");
  const p = useNuevoPin();
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);
  const listo = codigo.replace(/[^0-9a-z]/gi, "").length === 10 && p.listo;
  const enviar = async () => {
    if (!listo || enviando) return;
    setEnviando(true);
    setError(null);
    try {
      onRecuperado(await api.recuperarPin(codigo, p.nuevo));
    } catch (e) {
      setError(textoError(e));
      setEnviando(false);
    }
  };
  return (
    <FormularioDialogo titulo="Olvidé el PIN" onCancelar={onCancelar} onEnviar={() => void enviar()}>
      <p className="text-tenue">Escribe el código de recuperación que anotaste al crear el PIN.</p>
      <label className="block">
        <span className="text-sm text-tenue">Código de recuperación</span>
        <input
          autoFocus
          value={codigo}
          onChange={(e) => setCodigo(e.target.value.toUpperCase().slice(0, 13))}
          placeholder="ABCDE-12345"
          autoComplete="off"
          spellCheck={false}
          className="mt-1 h-11 w-full rounded-lg border border-borde-control bg-superficie px-3 text-center font-mono text-lg tracking-widest focus:border-acento"
        />
      </label>
      <CampoPin etiqueta="PIN nuevo (4 a 6 dígitos)" valor={p.nuevo} onCambiar={p.setNuevo} />
      <CampoPin etiqueta="Repite el PIN nuevo" valor={p.repetido} onCambiar={p.setRepetido} />
      {p.distinto && <p className="text-xs text-peligro">Los dos PIN nuevos no coinciden.</p>}
      <MensajeError texto={error} />
      <PieDialogo onCancelar={onCancelar} listo={listo} enviando={enviando} texto="Poner PIN nuevo" />
    </FormularioDialogo>
  );
}

/** El código de recuperación se ve una sola vez: al crear el PIN o al usar el anterior. */
export function CodigoRecuperacion({ codigo, onCerrar }: { codigo: string; onCerrar: () => void }) {
  return (
    <Dialogo titulo="Código de recuperación" onCerrar={onCerrar}>
      <p className="mt-2 text-tenue">Sirve para poner un PIN nuevo si olvidas el actual. No se vuelve a mostrar.</p>
      <p className="mt-4 rounded-lg border border-linea bg-suave py-4 text-center font-mono text-3xl font-semibold tracking-widest select-text">
        {codigo}
      </p>
      <p className="mt-4 rounded-lg bg-aviso-fondo px-3 py-2 font-medium text-aviso">
        Anótalo y guárdalo fuera del computador.
      </p>
      <Boton variante="primario" autoFocus onClick={onCerrar} className="mt-5 w-full">
        Ya lo anoté
      </Boton>
    </Dialogo>
  );
}
