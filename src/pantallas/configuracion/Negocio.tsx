import { Info } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { api, textoError, type DatosNegocio, type Negocio } from "../../api";
import { Columnas, Tarjeta } from "../../componentes/Ajustes";
import Boton from "../../componentes/Boton";
import { esCancelado, type ConPin } from "../../componentes/PedirPin";
import { pesos } from "../../formato";

const claseCampo = "h-10 w-full rounded-lg border border-borde-control px-3 focus:border-acento";

const VACIO: DatosNegocio = { nombre: "", rut: "", direccion: "", telefono: "", mensajePie: "" };

/** Las tres líneas de ejemplo de la vista previa: fijas, no salen de la base. */
const EJEMPLO: [string, number][] = [
  ["1 × Leche entera 1 L", 1190],
  ["0,520 kg Tomate", 1560],
  ["2 × Pan hallulla", 1800],
];

/** Configuración > Negocio: los datos que encabezan cada boleta, el logo y una vista previa. */
export default function NegocioConfig({ conPin, onGuardado }: { conPin: ConPin; onGuardado?: (n: Negocio) => void }) {
  const [datos, setDatos] = useState<DatosNegocio>(VACIO);
  const [logo, setLogo] = useState<string | null>(null);
  const [cargado, setCargado] = useState(false);
  const [mensaje, setMensaje] = useState<{ error: boolean; texto: string } | null>(null);
  const [ocupado, setOcupado] = useState(false);

  const mostrar = (n: Negocio) => {
    setDatos({
      nombre: n.nombre,
      rut: n.rut ?? "",
      direccion: n.direccion ?? "",
      telefono: n.telefono ?? "",
      mensajePie: n.mensajePie,
    });
    setLogo(n.logoPng);
  };

  useEffect(() => {
    api
      .obtenerNegocio()
      .then(mostrar)
      .catch((e) => setMensaje({ error: true, texto: textoError(e) }))
      .finally(() => setCargado(true));
  }, []);

  const ejecutar = async (accion: () => Promise<Negocio | null>, exito: string) => {
    if (ocupado) return;
    setOcupado(true);
    setMensaje(null);
    try {
      const n = await accion();
      if (n) {
        mostrar(n);
        setMensaje({ error: false, texto: exito });
        onGuardado?.(n);
      }
    } catch (e) {
      if (!esCancelado(e)) setMensaje({ error: true, texto: textoError(e) });
    } finally {
      setOcupado(false);
    }
  };

  const cambiar = (campo: keyof DatosNegocio) => (e: { target: { value: string } }) =>
    setDatos((d) => ({ ...d, [campo]: e.target.value }));

  if (!cargado) return <p className="text-tenue">Cargando…</p>;

  return (
    <Columnas
      izquierda={
        <Tarjeta titulo="Datos del negocio" ayuda="Encabezan cada boleta.">
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void ejecutar(
                () => conPin((pin) => api.guardarNegocio(datos, pin), { accion: "Guardar los datos del negocio" }),
                "Datos guardados.",
              );
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
            <div className="flex items-center gap-3">
              <Boton type="submit" variante="primario" disabled={ocupado}>
                Guardar datos
              </Boton>
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
                  void ejecutar(() => conPin((pin) => api.elegirLogo(pin), { accion: "Cambiar el logo" }), "Logo cargado.")
                }
              >
                Elegir logo…
              </Boton>
              {logo && (
                <Boton
                  variante="terciario"
                  disabled={ocupado}
                  onClick={() =>
                    void ejecutar(() => conPin((pin) => api.quitarLogo(pin), { accion: "Quitar el logo" }), "Logo quitado.")
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
      className="mx-auto w-full max-w-[280px] bg-papel px-4 py-5 font-mono text-[11.5px] leading-snug text-tinta shadow-s2"
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
      <span className="text-xs text-tenue">{etiqueta}</span>
      <div className="mt-1">{children}</div>
    </label>
  );
}
