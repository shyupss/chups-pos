import { useState } from "react";
import { api, textoError, type Categoria } from "../api";

interface Props {
  categorias: Categoria[];
  /** Se llama después de cada cambio para volver a leer la lista. */
  onCambio: () => Promise<void>;
  onCerrar: () => void;
}

const claseCampo = "h-10 min-w-0 flex-1 rounded-lg border border-linea bg-fondo px-3 outline-none focus:border-texto";
const claseBoton = "h-8 rounded-lg px-2 text-tenue hover:bg-fondo hover:text-texto";

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
        <button type="button" onClick={onCerrar} className={claseBoton}>
          Cerrar
        </button>
      </div>

      <form
        className="mt-4 flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          void crear();
        }}
      >
        <input
          autoFocus
          value={nueva}
          onChange={(e) => setNueva(e.target.value)}
          placeholder="Nueva categoría"
          aria-label="Nombre de la nueva categoría"
          maxLength={60}
          className={claseCampo}
        />
        <button
          type="submit"
          disabled={!nueva.trim()}
          className="h-10 rounded-lg bg-acento px-4 font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
        >
          Agregar
        </button>
      </form>

      {error && <p className="mt-3 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}

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
                <button type="submit" className={claseBoton}>
                  Guardar
                </button>
              </form>
            ) : eliminando === c.id ? (
              <>
                <span className="min-w-0 flex-1 truncate">¿Eliminar «{c.nombre}»?</span>
                <button
                  type="button"
                  autoFocus
                  onClick={() => void eliminar(c.id)}
                  className="h-8 rounded-lg bg-peligro/10 px-2 font-medium text-peligro"
                >
                  Eliminar
                </button>
                <button type="button" onClick={() => setEliminando(null)} className={claseBoton}>
                  No
                </button>
              </>
            ) : (
              <>
                <span className="min-w-0 flex-1 truncate">{c.nombre}</span>
                <span className="shrink-0 text-xs text-tenue tabular-nums">
                  {c.productos} {c.productos === 1 ? "producto" : "productos"}
                </span>
                <button
                  type="button"
                  onClick={() => {
                    setEliminando(null);
                    setEditando({ id: c.id, nombre: c.nombre });
                  }}
                  className={claseBoton}
                >
                  Renombrar
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setEditando(null);
                    setEliminando(c.id);
                  }}
                  className="h-8 rounded-lg px-2 text-tenue hover:bg-peligro/10 hover:text-peligro"
                >
                  Eliminar
                </button>
              </>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
