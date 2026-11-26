import { Info } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { api, textoError, type DatosNegocio, type Negocio } from "../../api";
import { Columnas, Tarjeta } from "../../componentes/Ajustes";
import Boton from "../../componentes/Boton";
import Dialogo from "../../componentes/Dialogo";
import { esCancelado, type ConPin } from "../../componentes/PedirPin";
import { pesos } from "../../formato";
import { useGuardiaSalida } from "../../salida";
import { conLogo, DATOS_VACIOS, desdeGuardado, hayCambios, type EstadoNegocio } from "./negocio-estado";

const claseCampo = "h-11 w-full rounded-lg border border-borde-control px-3 focus:border-acento";

/** Las tres líneas de ejemplo de la vista previa: fijas, no salen de la base. */
const EJEMPLO: [string, number][] = [
  ["1 × Leche entera 1 L", 1190],
  ["0,520 kg Tomate", 1560],
  ["2 × Pan hallulla", 1800],
];

/** Configuración > Negocio: los datos que encabezan cada boleta, el logo y una vista previa. */
export default function NegocioConfig({ conPin, onGuardado }: { conPin: ConPin; onGuardado?: (n: Negocio) => void }) {
  const [estado, setEstado] = useState<EstadoNegocio>({ datos: DATOS_VACIOS, guardados: DATOS_VACIOS, logo: null });
  const { datos, logo } = estado;
  const [cargado, setCargado] = useState(false);
  const [mensaje, setMensaje] = useState<{ error: boolean; texto: string } | null>(null);
  const [ocupado, setOcupado] = useState(false);
  // Salir con cambios sin guardar: lo que se iba a hacer queda aquí mientras se pregunta.
  const [salida, setSalida] = useState<(() => void) | null>(null);
  const pendientes = cargado && hayCambios(datos, estado.guardados);

  useEffect(() => {
    api
      .obtenerNegocio()
      .then((n) => setEstado(desdeGuardado(n)))
      .catch((e) => setMensaje({ error: true, texto: textoError(e) }))
      .finally(() => setCargado(true));
  }, []);

  useGuardiaSalida(pendientes, (continuar) => setSalida(() => continuar));

  /**
   * Corre una acción protegida con el PIN. `aplicar` decide qué cambia en pantalla: guardar los
   * datos lo reemplaza todo; el logo solo cambia el logo. Devuelve si se hizo.
   */
  const ejecutar = async (
    accion: () => Promise<Negocio | null>,
    aplicar: (e: EstadoNegocio, n: Negocio) => EstadoNegocio,
    exito: string,
  ): Promise<boolean> => {
    if (ocupado) return false;
    setOcupado(true);
    setMensaje(null);
    try {
      const n = await accion();
      if (!n) return false;
      setEstado((e) => aplicar(e, n));
      setMensaje({ error: false, texto: exito });
      onGuardado?.(n);
      return true;
    } catch (e) {
      if (!esCancelado(e)) setMensaje({ error: true, texto: textoError(e) });
      return false;
    } finally {
      setOcupado(false);
    }
  };

  const guardar = () =>
    ejecutar(
      () => conPin((pin) => api.guardarNegocio(datos, pin), { accion: "Guardar los datos del negocio" }),
      (_, n) => desdeGuardado(n),
      "Datos guardados.",
    );

  const cambiar = (campo: keyof DatosNegocio) => (e: { target: { value: string } }) => {
    // Un "Datos guardados." anterior ya no vale mientras se escribe.
    setMensaje((m) => (m && !m.error ? null : m));
    setEstado((s) => ({ ...s, datos: { ...s.datos, [campo]: e.target.value } }));
  };

  if (!cargado) return <p className="text-tenue">Cargando…</p>;

  return (
    <Columnas
      izquierda={
        <Tarjeta titulo="Datos del negocio" ayuda="Encabezan cada boleta.">
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void guardar();
            }}
            className="flex flex-col gap-4"
          >
            <Campo etiqueta="Nombre del negocio (obligatorio)">
              <input value={datos.nombre} onChange={cambiar("nombre")} maxLength={60} className={claseCampo} />
            </Campo>
            <div className="grid grid-cols-2 gap-4">
              <Campo etiqueta="RUT (opcional)">
                <input
                  value={datos.rut ?? ""}
                  onChange={cambiar("rut")}
                  placeholder="12.345.678-5"
                  maxLength={16}
                  className={claseCampo}
                />
              </Campo>
              <Campo etiqueta="Teléfono (opcional)">
                <input value={datos.telefono ?? ""} onChange={cambiar("telefono")} maxLength={30} className={claseCampo} />
              </Campo>
            </div>
            <Campo etiqueta="Dirección (opcional)">
              <input value={datos.direccion ?? ""} onChange={cambiar("direccion")} maxLength={120} className={claseCampo} />
            </Campo>
            <Campo etiqueta="Mensaje al pie">
              <input value={datos.mensajePie} onChange={cambiar("mensajePie")} maxLength={200} className={claseCampo} />
            </Campo>
            <p className="flex items-start gap-2 rounded-lg bg-info-fondo px-3 py-2 text-[13px] text-info">
              <Info size={16} className="mt-0.5 shrink-0" aria-hidden />
              La boleta es un comprobante interno: no reemplaza la boleta del SII.
            </p>
            <div className="flex flex-wrap items-center gap-3">
              <Boton type="submit" variante="primario" disabled={ocupado}>
                {pendientes ? "Guardar cambios" : "Guardar datos"}
              </Boton>
              {pendientes && (
                <span role="status" className="text-aviso">
                  Tienes cambios sin guardar
                </span>
              )}
              {mensaje && (
                <span role="status" className={mensaje.error ? "text-peligro" : "text-exito"}>
                  {mensaje.texto}
                </span>
              )}
            </div>
          </form>
        </Tarjeta>
      }
      derecha={
        <>
          <Tarjeta titulo="Logo" ayuda="Se imprime en blanco y negro, hasta 384 × 160 puntos.">
            <div className="flex flex-wrap items-center gap-3">
              <div className="flex min-h-20 min-w-40 items-center justify-center rounded-lg border border-linea bg-papel p-2">
                {logo ? (
                  <img
                    src={`data:image/png;base64,${logo}`}
                    alt="Logo del negocio como saldrá en la boleta"
                    className="max-w-full [image-rendering:pixelated]"
                  />
                ) : (
                  <span className="text-xs text-tinta">Sin logo</span>
                )}
              </div>
              <Boton
                disabled={ocupado}
                onClick={() =>
                  void ejecutar(
                    () => conPin((pin) => api.elegirLogo(pin), { accion: "Cambiar el logo" }),
                    conLogo,
                    "Logo cargado.",
                  )
                }
              >
                Elegir logo…
              </Boton>
              {logo && (
                <Boton
                  variante="terciario"
                  disabled={ocupado}
                  onClick={() =>
                    void ejecutar(
                      () => conPin((pin) => api.quitarLogo(pin), { accion: "Quitar el logo" }),
                      conLogo,
                      "Logo quitado.",
                    )
                  }
                >
                  Quitar logo
                </Boton>
              )}
            </div>
          </Tarjeta>
          <Tarjeta titulo="Vista previa de la boleta" ayuda="Vista aproximada, con datos de ejemplo: se actualiza mientras escribes.">
            <VistaBoleta datos={datos} logo={logo} />
          </Tarjeta>
          {salida && (
            <Dialogo titulo="¿Guardar los cambios?" onCerrar={() => setSalida(null)} ancho="w-[30rem]">
              <p className="mt-2 text-tenue">Cambiaste los datos del negocio y todavía no los guardas.</p>
              <div className="mt-6 flex flex-wrap justify-end gap-2">
                <Boton variante="terciario" onClick={() => setSalida(null)}>
                  Seguir editando
                </Boton>
                <Boton
                  onClick={() => {
                    const continuar = salida;
                    setSalida(null);
                    setEstado((e) => ({ ...e, datos: e.guardados }));
                    continuar();
                  }}
                >
                  Descartar
                </Boton>
                <Boton
                  variante="primario"
                  disabled={ocupado}
                  onClick={async () => {
                    const continuar = salida;
                    setSalida(null);
                    if (await guardar()) continuar();
                  }}
                >
                  Guardar cambios
                </Boton>
              </div>
            </Dialogo>
          )}
        </>
      }
    />
  );
}

/** El papel de la boleta, hecho con HTML y datos de ejemplo: no llama al backend ni imprime nada. */
function VistaBoleta({ datos, logo }: { datos: DatosNegocio; logo: string | null }) {
  const total = EJEMPLO.reduce((s, [, monto]) => s + monto, 0);
  return (
    <div
      role="img"
      aria-label="Vista aproximada de la boleta"
      className="mx-auto w-full max-w-[280px] bg-papel px-4 py-5 font-mono text-[13px] leading-snug text-tinta shadow-s2"
    >
      <div className="text-center">
        {logo && (
          <img src={`data:image/png;base64,${logo}`} alt="" className="mx-auto mb-2 max-h-12 [image-rendering:pixelated]" />
        )}
        <div className="text-[13px] font-bold break-words">{datos.nombre.trim() || "Nombre del negocio"}</div>
        {datos.rut?.trim() && <div>RUT {datos.rut.trim()}</div>}
        {datos.direccion?.trim() && <div className="break-words">{datos.direccion.trim()}</div>}
        {datos.telefono?.trim() && <div>Tel. {datos.telefono.trim()}</div>}
      </div>
      <hr className="my-2 border-t border-dashed border-tinta/50" />
      <ul className="flex flex-col gap-0.5">
        {EJEMPLO.map(([nombre, monto]) => (
          <li key={nombre} className="flex justify-between gap-2">
            <span className="truncate">{nombre}</span>
            <span className="shrink-0">{pesos(monto)}</span>
          </li>
        ))}
      </ul>
      <hr className="my-2 border-t border-dashed border-tinta/50" />
      <div className="flex justify-between text-[13px] font-bold">
        <span>TOTAL</span>
        <span>{pesos(total)}</span>
      </div>
      {datos.mensajePie.trim() && <div className="mt-3 text-center break-words">{datos.mensajePie.trim()}</div>}
    </div>
  );
}

function Campo({ etiqueta, children }: { etiqueta: string; children: ReactNode }) {
  return (
    <label className="block">
      <span className="text-sm text-tenue">{etiqueta}</span>
      <div className="mt-1">{children}</div>
    </label>
  );
}
