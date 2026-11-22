import { useEffect, useState } from "react";
import { api, textoError, type EstadoIntegridad } from "../api";
import Boton from "./Boton";
import { fechaHora } from "../formato";
import Dialogo from "./Dialogo";
import { esCancelado, useConPin } from "./PedirPin";

/**
 * Después de un cierre brusco (un corte de luz), Rust revisa la base al arrancar. Si encontró un
 * problema, esto ofrece restaurar el último respaldo sano; si todo está bien, no aparece.
 */
export default function AvisoIntegridad() {
  const conPin = useConPin();
  const [estado, setEstado] = useState<EstadoIntegridad | null>(null);
  const [descartado, setDescartado] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [restaurando, setRestaurando] = useState(false);

  useEffect(() => {
    api
      .estadoIntegridad()
      .then(setEstado)
      .catch(() => {});
  }, []);

  if (!estado || estado.problemas.length === 0 || descartado) return null;
  const sano = estado.ultimoSano;

  const restaurar = async () => {
    setRestaurando(true);
    setError(null);
    try {
      // Si sale bien, la app se reinicia y este diálogo desaparece.
      await conPin((pin) => api.restaurarUltimoSano(pin), { accion: "Restaurar el último respaldo sano" });
    } catch (e) {
      if (!esCancelado(e)) setError(textoError(e));
      setRestaurando(false);
    }
  };

  return (
    <Dialogo titulo="La base de datos tiene un problema" onCerrar={() => setDescartado(true)} ancho="w-[34rem]">
      <p className="mt-3">
        CHUPS POS se cerró de golpe la última vez (por ejemplo, por un corte de luz) y al volver a abrirlo se encontró un
        problema en la base de datos. Puede faltar lo último que se registró.
      </p>
      {sano ? (
        <p className="mt-3 rounded-lg bg-info-fondo px-3 py-2 text-info">
          Hay un respaldo sano del {fechaHora(sano.fecha)}. Si lo restauras, lo vendido después de esa fecha no estará; la base dañada
          se guarda aparte en la carpeta de respaldos.
        </p>
      ) : (
        <p className="mt-3 rounded-lg bg-aviso-fondo px-3 py-2 text-aviso">
          No hay un respaldo sano para restaurar. No borres nada y contacta a quien te vendió el sistema; el detalle quedó en
          el registro (Configuración &gt; Acerca de &gt; Abrir carpeta de registros).
        </p>
      )}
      <details className="mt-3 text-[13px] text-tenue">
        <summary className="cursor-pointer">Detalle técnico</summary>
        <ul className="mt-1 list-disc pl-5 select-text">
          {estado.problemas.map((p) => (
            <li key={p}>{p}</li>
          ))}
        </ul>
      </details>
      {error && <p className="mt-3 rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}
      <div className="mt-5 flex gap-2">
        <Boton variante="terciario" onClick={() => setDescartado(true)} className="flex-1">
          {sano ? "Seguir sin restaurar" : "Entendido"}
        </Boton>
        {sano && (
          <Boton variante="primario" disabled={restaurando} onClick={() => void restaurar()} className="flex-[2]">
            {restaurando ? "Restaurando…" : "Restaurar el respaldo sano"}
          </Boton>
        )}
      </div>
    </Dialogo>
  );
}
