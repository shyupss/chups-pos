import { useEffect, useState, type ReactNode } from "react";
import { api, textoError, type DatosNegocio, type Negocio } from "../../api";

const claseCampo =
  "h-10 w-full rounded-lg border border-linea bg-fondo px-3 outline-none focus:border-texto";

const VACIO: DatosNegocio = { nombre: "", rut: "", direccion: "", telefono: "", mensajePie: "" };

/** Configuración > Negocio: los datos que encabezan cada boleta, más el logo. */
export default function NegocioConfig() {
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
      }
    } catch (e) {
      setMensaje({ error: true, texto: textoError(e) });
    } finally {
      setOcupado(false);
    }
  };

  const cambiar = (campo: keyof DatosNegocio) => (e: { target: { value: string } }) =>
    setDatos((d) => ({ ...d, [campo]: e.target.value }));

  if (!cargado) return <p className="text-tenue">Cargando…</p>;

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        void ejecutar(() => api.guardarNegocio(datos), "Datos guardados.");
      }}
      className="space-y-4"
    >
      <p className="text-tenue">
        Encabezan cada boleta. La boleta es un comprobante interno: no reemplaza la boleta del SII.
      </p>
      <div className="grid grid-cols-2 gap-3">
        <Campo etiqueta="Nombre del negocio (obligatorio)" ancho>
          <input value={datos.nombre} onChange={cambiar("nombre")} maxLength={60} className={claseCampo} />
        </Campo>
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
        <Campo etiqueta="Dirección (opcional)" ancho>
          <input value={datos.direccion ?? ""} onChange={cambiar("direccion")} maxLength={120} className={claseCampo} />
        </Campo>
        <Campo etiqueta="Mensaje al pie" ancho>
          <input value={datos.mensajePie} onChange={cambiar("mensajePie")} maxLength={200} className={claseCampo} />
        </Campo>
      </div>

      <div>
        <span className="text-xs text-tenue">Logo (se imprime en blanco y negro, hasta 384 × 160 puntos)</span>
        <div className="mt-1 flex flex-wrap items-center gap-3">
          <div className="flex min-h-20 min-w-40 items-center justify-center rounded-lg border border-linea bg-papel p-2">
            {logo ? (
              <img
                src={`data:image/png;base64,${logo}`}
                alt="Logo del negocio como saldrá en la boleta"
                className="max-w-full [image-rendering:pixelated]"
              />
            ) : (
              <span className="text-xs text-tenue">Sin logo</span>
            )}
          </div>
          <button
            type="button"
            disabled={ocupado}
            onClick={() => void ejecutar(() => api.elegirLogo(), "Logo cargado.")}
            className="h-10 rounded-lg border border-linea px-4 hover:bg-fondo disabled:opacity-50"
          >
            Elegir logo…
          </button>
          {logo && (
            <button
              type="button"
              disabled={ocupado}
              onClick={() => void ejecutar(() => api.quitarLogo(), "Logo quitado.")}
              className="h-10 rounded-lg px-4 text-tenue hover:bg-peligro/10 hover:text-peligro disabled:opacity-50"
            >
              Quitar logo
            </button>
          )}
        </div>
      </div>

      <div className="flex items-center gap-3">
        <button
          type="submit"
          disabled={ocupado}
          className="h-10 rounded-lg bg-acento px-5 font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
        >
          Guardar datos
        </button>
        {mensaje && (
          <span role="status" className={mensaje.error ? "text-peligro" : "text-exito"}>
            {mensaje.texto}
          </span>
        )}
      </div>
    </form>
  );
}

function Campo({ etiqueta, ancho, children }: { etiqueta: string; ancho?: boolean; children: ReactNode }) {
  return (
    <label className={`block ${ancho ? "col-span-2" : ""}`}>
      <span className="text-xs text-tenue">{etiqueta}</span>
      <div className="mt-1">{children}</div>
    </label>
  );
}
