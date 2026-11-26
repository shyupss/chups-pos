import Boton from "./Boton";
import { useState } from "react";
import { api, textoError, type Categoria } from "../api";

interface Props {
  categorias: Categoria[];
  /** Se llama después de cada cambio para volver a leer la lista. */
  onCambio: () => Promise<void>;
  onCerrar: () => void;
}

const claseCampo = "h-11 min-w-0 flex-1 rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento";

/** Crear, renombrar y eliminar categorías. Vive en el panel derecho de Productos. */
export default function PanelCategorias({ categorias, onCambio, onCerrar }: Props) {
  const [nueva, setNueva] = useState("");
  const [editando, setEditando] = useState<{ id: number; nombre: string } | null>(null);
  const [eliminando, setEliminando] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  const ejecutar = async (accion: () => Promise<unknown>) => {
    setError(null);
    try {
      await accion();
      await onCambio();
      return true;
    } catch (e) {
      setError(textoError(e));
      return false;
    }
  };

  const crear = async () => {
    if (!nueva.trim()) return;
    if (await ejecutar(() => api.crearCategoria(nueva))) setNueva("");
  };
  const renombrar = async () => {
    if (!editando) return;
    if (await ejecutar(() => api.renombrarCategoria(editando.id, editando.nombre))) setEditando(null);
  };
  const eliminar = async (id: number) => {
    await ejecutar(() => api.eliminarCategoria(id));
    setEliminando(null);
  };

  return (
    <div
      className="flex h-full flex-col p-6"
      onKeyDown={(e) => {
        if (e.key !== "Escape") return;
        if (editando || eliminando !== null) {
          setEditando(null);
          setEliminando(null);
        } else {
          onCerrar();
        }
      }}
    >
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold">Categorías</h2>
        <Boton variante="terciario" tamano="chico" onClick={onCerrar}>
          Cerrar
        </Boton>
      </div>

      <form
        className="mt-4 flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          void crear();
        }}
      >
        <label className="min-w-0 flex-1">
          <span className="text-sm text-tenue">Nombre de la categoría nueva</span>
          <input
            autoFocus
            value={nueva}
            onChange={(e) => setNueva(e.target.value)}
            placeholder="Por ejemplo, Bebidas"
            maxLength={60}
            className={`mt-1 ${claseCampo}`}
          />
        </label>
        <Boton type="submit" variante="primario" disabled={!nueva.trim()} className="self-end">
          Agregar categoría
        </Boton>
      </form>

      {error && <p className="mt-3 rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}

      <ul className="mt-4 min-h-0 flex-1 overflow-y-auto">
        {categorias.length === 0 && <li className="py-6 text-center text-tenue">Todavía no hay categorías.</li>}
        {categorias.map((c) => (
          <li key={c.id} className="flex min-h-12 items-center gap-2 border-t border-linea py-1">
            {editando?.id === c.id ? (
              <form
                className="flex flex-1 gap-2"
                onSubmit={(e) => {
                  e.preventDefault();
                  void renombrar();
                }}
              >
                <input
                  autoFocus
                  value={editando.nombre}
                  onChange={(e) => setEditando({ id: c.id, nombre: e.target.value })}
                  aria-label={`Nuevo nombre de ${c.nombre}`}
                  maxLength={60}
                  className={claseCampo}
                />
                <Boton type="submit" tamano="chico">
                  Guardar nombre
                </Boton>
              </form>
            ) : eliminando === c.id ? (
              <>
                <span className="min-w-0 flex-1 truncate">¿Eliminar «{c.nombre}»?</span>
                <Boton variante="peligro" tamano="chico" autoFocus onClick={() => void eliminar(c.id)}>
                  Eliminar
                </Boton>
                <Boton variante="terciario" tamano="chico" onClick={() => setEliminando(null)}>
                  No
                </Boton>
              </>
            ) : (
              <>
                <span className="min-w-0 flex-1 truncate" title={c.nombre}>
                  {c.nombre}
                </span>
                <span className="shrink-0 text-xs text-tenue tabular-nums">
                  {c.productos} {c.productos === 1 ? "producto" : "productos"}
                </span>
                <Boton
                  variante="terciario"
                  tamano="chico"
                  onClick={() => {
                    setEliminando(null);
                    setEditando({ id: c.id, nombre: c.nombre });
                  }}
                >
                  Renombrar
                </Boton>
                <Boton
                  variante="terciario"
                  tamano="chico"
                  onClick={() => {
                    setEditando(null);
                    setEliminando(c.id);
                  }}
                >
                  Eliminar
                </Boton>
              </>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
