import { Clock, TriangleAlert } from "lucide-react";
import type { EstadoLicencia } from "../api";
import Boton from "./Boton";

/**
 * Aviso arriba de cada pantalla cuando la licencia lo pide: la gracia después de la prueba, el
 * modo consulta, una licencia que no vale en este equipo o el reloj atrasado. En prueba y con la
 * licencia activa no se muestra (la prueba se ve, discreta, en la barra lateral).
 */
export default function AvisoLicencia({ estado, onActivar }: { estado: EstadoLicencia | null; onActivar: () => void }) {
  if (!estado) return null;
  const dias = estado.diasRestantes === 1 ? "1 día" : `${estado.diasRestantes ?? 0} días`;
  const aviso =
    estado.modo === "consulta"
      ? {
          peligro: true,
          texto:
            "Modo consulta: la prueba terminó. Puedes ver, exportar y respaldar tus datos, cobrar fiados y cerrar el turno, pero no abrir la caja ni vender.",
        }
      : estado.modo === "gracia"
        ? {
            peligro: false,
            texto: `La prueba terminó: quedan ${dias} para activar la licencia. Después solo se podrá consultar.`,
          }
        : estado.errorLicencia && estado.modo !== "activa"
          ? { peligro: true, texto: `La licencia guardada no vale en este computador: ${estado.errorLicencia}` }
          : null;

  if (!aviso && !estado.relojAtrasado) return null;
  return (
    <div className="flex shrink-0 flex-col gap-2 px-6 pt-4">
      {aviso && (
        <div
          role="alert"
          className={`flex flex-wrap items-center gap-3 rounded-lg px-4 py-2.5 ${
            aviso.peligro ? "bg-peligro-fondo text-peligro" : "bg-aviso-fondo text-aviso"
          }`}
        >
          <TriangleAlert size={18} className="shrink-0" aria-hidden />
          <span className="min-w-0 flex-1">{aviso.texto}</span>
          <Boton tamano="chico" onClick={onActivar}>
            Activar licencia
          </Boton>
        </div>
      )}
      {estado.relojAtrasado && estado.modo !== "activa" && (
        <div role="status" className="flex items-center gap-3 rounded-lg bg-aviso-fondo px-4 py-2.5 text-aviso">
          <Clock size={18} className="shrink-0" aria-hidden />
          El reloj del computador está atrasado más de dos días. Corrígelo en Windows: atrasarlo no devuelve días de prueba.
        </div>
      )}
    </div>
  );
}
