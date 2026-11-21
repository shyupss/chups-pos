import { useEffect, useState } from "react";
import { api, type EstadoSistema } from "../api";
import Dato from "../componentes/Dato";

// Pantalla provisoria de la fase 0: confirma que Vista -> Controlador -> Modelo -> SQLite
// funciona de punta a punta y muestra cuánto tardó el arranque.
export default function Inicio() {
  const [estado, setEstado] = useState<EstadoSistema | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.estadoSistema().then(setEstado).catch((e) => setError(String(e)));
  }, []);

  if (error) {
    return <p className="text-red-700">No se pudo leer el estado del sistema: {error}</p>;
  }
  if (!estado) {
    return <p className="text-slate-500">Cargando…</p>;
  }

  return (
    <section className="grid max-w-3xl grid-cols-2 gap-3">
      <Dato etiqueta="Versión" valor={estado.version} />
      <Dato etiqueta="Arranque del backend" valor={`${estado.msInicio} ms`} />
      <Dato etiqueta="Esquema de base de datos" valor={`v${estado.versionEsquema}`} />
      <Dato etiqueta="Productos" valor={estado.totalProductos} />
      <div className="col-span-2">
        <Dato etiqueta="Archivo de datos" valor={estado.rutaBaseDatos} />
      </div>
    </section>
  );
}
