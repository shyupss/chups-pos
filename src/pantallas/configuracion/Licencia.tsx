import { BadgeCheck, Clock, Copy, FileKey, Info, TriangleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { api, textoError, type EstadoLicencia } from "../../api";
import { Columnas, Tarjeta } from "../../componentes/Ajustes";
import Boton from "../../componentes/Boton";

const fecha = (iso: string | null) =>
  iso ? new Date(iso).toLocaleDateString("es-CL", { day: "numeric", month: "long", year: "numeric" }) : "—";

const dias = (n: number | null) => (n === 1 ? "1 día" : `${n ?? 0} días`);

/** Insignia del estado, con los pares de color de éxito, información, aviso y peligro. */
export function InsigniaLicencia({ estado }: { estado: EstadoLicencia }) {
  const [texto, clase, Icono] =
    estado.modo === "activa"
      ? ["Licencia activa", "bg-exito-fondo text-exito", BadgeCheck]
      : estado.modo === "prueba"
        ? [`Prueba: quedan ${dias(estado.diasRestantes)}`, "bg-info-fondo text-info", Clock]
        : estado.modo === "gracia"
          ? [`Prueba terminada: ${dias(estado.diasRestantes)} de gracia`, "bg-aviso-fondo text-aviso", TriangleAlert]
          : ["Modo consulta", "bg-peligro-fondo text-peligro", TriangleAlert];
  return (
    <span className={`flex w-fit items-center gap-2 rounded-lg px-3 py-2 font-medium ${clase}`}>
      <Icono size={16} aria-hidden /> {texto}
    </span>
  );
}

/** Configuración > Licencia: estado, código de solicitud y activar con el texto o el archivo. */
export default function LicenciaConfig({
  estado,
  onCambio,
}: {
  estado: EstadoLicencia | null;
  onCambio: (e: EstadoLicencia) => void;
}) {
  const [texto, setTexto] = useState("");
  const [mensaje, setMensaje] = useState<{ error: boolean; texto: string } | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [copiado, setCopiado] = useState(false);
  // El nombre que sale en las boletas (Configuración > Negocio): puede ser distinto del de la licencia.
  const [nombreBoleta, setNombreBoleta] = useState<string | null>(null);
  useEffect(() => {
    api
      .obtenerNegocio()
      .then((n) => setNombreBoleta(n.nombre))
      .catch(() => {});
  }, []);

  if (!estado) return <p className="text-tenue">Cargando…</p>;

  const activar = async (accion: () => Promise<EstadoLicencia | null>) => {
    if (ocupado) return;
    setOcupado(true);
    setMensaje(null);
    try {
      const nuevo = await accion();
      if (nuevo) {
        onCambio(nuevo);
        setTexto("");
        setMensaje({ error: false, texto: `Licencia activada para ${nuevo.licencia?.negocio ?? "este negocio"}.` });
      }
    } catch (e) {
      setMensaje({ error: true, texto: textoError(e) });
    } finally {
      setOcupado(false);
    }
  };

  const copiar = () => {
    void navigator.clipboard
      .writeText(estado.codigoSolicitud)
      .then(() => {
        setCopiado(true);
        setTimeout(() => setCopiado(false), 2000);
      })
      .catch(() => {});
  };

  const activa = estado.modo === "activa";
  const lic = estado.licencia;

  return (
    <Columnas
      izquierda={
        <>
          <Tarjeta titulo="Estado de la licencia" ayuda="La licencia es permanente: no vence ni deja de valer con las actualizaciones.">
            <InsigniaLicencia estado={estado} />
            {activa && lic ? (
              <>
              <dl className="grid grid-cols-[10rem_1fr] gap-x-4 gap-y-2">
                <dt className="text-tenue">Emitida a</dt>
                <dd className="min-w-0 break-words">{lic.negocio}</dd>
                {nombreBoleta !== null && nombreBoleta.trim() !== "" && nombreBoleta.trim() !== lic.negocio.trim() && (
                  <>
                    <dt className="text-tenue">Nombre en las boletas</dt>
                    <dd className="min-w-0 break-words">{nombreBoleta}</dd>
                  </>
                )}
                <dt className="text-tenue">RUT</dt>
                <dd>{lic.rut ?? "—"}</dd>
                <dt className="text-tenue">Activada el</dt>
                <dd>{fecha(lic.activadaEn)}</dd>
                <dt className="text-tenue">Emitida el</dt>
                <dd>{fecha(`${lic.emitidaEn}T12:00:00Z`)}</dd>
                <dt className="text-tenue">Equipos</dt>
                <dd>{lic.equipos}</dd>
                <dt className="text-tenue">N° de licencia</dt>
                <dd className="font-mono text-[13px] select-text">{lic.id}</dd>
              </dl>
              <p className="text-[13px] text-tenue">
                Este nombre quedó grabado al emitir la licencia. Cambiar los datos del negocio en Configuración &gt; Negocio solo cambia la
                boleta; la licencia sigue valiendo.
              </p>
              <p className="text-[13px] text-tenue">
                ¿Cambió el dueño o la razón social? Pide al vendedor una licencia nueva con el código de solicitud.
              </p>
              </>
            ) : (
              <dl className="grid grid-cols-[10rem_1fr] gap-x-4 gap-y-2">
                <dt className="text-tenue">Inicio de la prueba</dt>
                <dd>{fecha(estado.inicioPrueba)}</dd>
                <dt className="text-tenue">Fin de la prueba</dt>
                <dd>{fecha(estado.finPrueba)}</dd>
                <dt className="text-tenue">Fin de la gracia</dt>
                <dd>{fecha(estado.finGracia)}</dd>
              </dl>
            )}
            {estado.modo === "gracia" && (
              <p className="rounded-lg bg-aviso-fondo px-3 py-2 text-[13px] text-aviso">
                La prueba terminó. Todo sigue funcionando {dias(estado.diasRestantes)} más; después, sin licencia, solo se podrá
                consultar.
              </p>
            )}
            {estado.modo === "consulta" && (
              <p className="rounded-lg bg-peligro-fondo px-3 py-2 text-[13px] text-peligro">
                Sin licencia no se puede abrir la caja ni registrar ventas. Puedes ver y exportar todos tus datos, respaldar y
                restaurar, cobrar fiados y cerrar el turno abierto. Nada se pierde: al activar la licencia todo sigue igual.
              </p>
            )}
            {estado.errorLicencia && (
              <p role="alert" className="flex items-start gap-2 rounded-lg bg-peligro-fondo px-3 py-2 text-[13px] text-peligro">
                <TriangleAlert size={16} className="mt-0.5 shrink-0" aria-hidden />
                La licencia guardada no vale en este computador: {estado.errorLicencia}
              </p>
            )}
            {estado.relojAtrasado && (
              <p className="flex items-start gap-2 rounded-lg bg-aviso-fondo px-3 py-2 text-[13px] text-aviso">
                <Clock size={16} className="mt-0.5 shrink-0" aria-hidden />
                El reloj del computador está atrasado. Corrígelo en Windows: atrasarlo no devuelve días de prueba.
              </p>
            )}
          </Tarjeta>
          {!activa && (
            <Tarjeta titulo="Cómo conseguir la licencia">
              <p className="flex items-start gap-2 text-tenue">
                <Info size={16} className="mt-0.5 shrink-0 text-info" aria-hidden />
                <span className="select-text">{estado.contacto}</span>
              </p>
            </Tarjeta>
          )}
        </>
      }
      derecha={
        <Tarjeta
          titulo={activa ? "Este equipo" : "Activar la licencia"}
          ayuda={
            activa
              ? "Para usar CHUPS POS en otro computador, pide una licencia para ese equipo con su propio código."
              : "Funciona sin internet: envía el código de solicitud y pega o carga la licencia que te respondan."
          }
        >
          <div>
            <div className="text-sm font-medium">{activa ? "Código de este equipo" : "1. Código de solicitud"}</div>
            <div className="mt-1 flex flex-wrap items-center gap-2">
              <code className="rounded-lg border border-linea bg-suave px-3 py-2 font-mono text-[15px] tracking-wide select-text">
                {estado.codigoSolicitud}
              </code>
              <Boton icono={Copy} tamano="chico" onClick={copiar}>
                {copiado ? "Copiado" : "Copiar"}
              </Boton>
            </div>
          </div>
          {!activa && (
            <form
              onSubmit={(e) => {
                e.preventDefault();
                void activar(() => api.activarLicencia(texto));
              }}
              className="flex flex-col gap-3 border-t border-linea pt-4"
            >
              <label className="block">
                <span className="text-sm font-medium">2. Pega la licencia</span>
                <textarea
                  value={texto}
                  onChange={(e) => setTexto(e.target.value)}
                  placeholder="CHUPS-XXXXX-XXXXX-…"
                  spellCheck={false}
                  rows={4}
                  className="mt-1 w-full resize-none rounded-lg border border-borde-control p-3 font-mono text-[13px] focus:border-acento"
                />
              </label>
              <div className="flex flex-wrap items-center gap-2">
                <Boton type="submit" variante="primario" disabled={ocupado || !texto.trim()}>
                  Activar
                </Boton>
                <Boton icono={FileKey} disabled={ocupado} onClick={() => void activar(api.cargarArchivoLicencia)}>
                  Cargar archivo…
                </Boton>
              </div>
            </form>
          )}
          {mensaje && (
            <p role={mensaje.error ? "alert" : "status"} className={mensaje.error ? "text-peligro" : "text-exito"}>
              {mensaje.texto}
            </p>
          )}
        </Tarjeta>
      }
    />
  );
}
