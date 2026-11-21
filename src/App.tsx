import { useCallback, useEffect, useState } from "react";
import Caja from "./pantallas/Caja";
import Productos from "./pantallas/Productos";
import Inicio from "./pantallas/Inicio";

type Pantalla = "caja" | "productos" | "sistema";

const PESTANAS: { id: Pantalla; nombre: string; tecla: string }[] = [
  { id: "caja", nombre: "Caja", tecla: "F1" },
  { id: "productos", nombre: "Productos", tecla: "F2" },
  { id: "sistema", nombre: "Sistema", tecla: "F9" },
];

export default function App() {
  const [pantalla, setPantalla] = useState<Pantalla>("caja");
  // Código escaneado en la caja que no existe: se abre Productos con el formulario listo.
  const [codigoNuevo, setCodigoNuevo] = useState<string | null>(null);

  useEffect(() => {
    const alPresionar = (e: KeyboardEvent) => {
      const p = PESTANAS.find((t) => t.tecla === e.key);
      if (p) {
        e.preventDefault();
        setPantalla(p.id);
      }
    };
    window.addEventListener("keydown", alPresionar);
    return () => window.removeEventListener("keydown", alPresionar);
  }, []);

  const crearDesdeCaja = useCallback((codigo: string) => {
    setCodigoNuevo(codigo);
    setPantalla("productos");
  }, []);

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center gap-6 border-b border-slate-200 bg-white px-6">
        <h1 className="py-3 text-lg font-semibold">CHUPS POS</h1>
        <nav className="flex gap-1">
          {PESTANAS.map((t) => (
            <button
              key={t.id}
              onClick={() => setPantalla(t.id)}
              className={`border-b-2 px-4 py-3 text-sm ${
                pantalla === t.id
                  ? "border-slate-900 font-semibold"
                  : "border-transparent text-slate-500 hover:text-slate-900"
              }`}
            >
              {t.nombre} <span className="ml-1 text-xs text-slate-400">{t.tecla}</span>
            </button>
          ))}
        </nav>
      </header>
      <main className="min-h-0 flex-1">
        {/* Caja queda montada siempre: cambiar de pestaña no borra el carrito. */}
        <div className={pantalla === "caja" ? "h-full" : "hidden"}>
          <Caja activa={pantalla === "caja"} onCrearProducto={crearDesdeCaja} />
        </div>
        {pantalla === "productos" && (
          <Productos codigoNuevo={codigoNuevo} onCodigoUsado={() => setCodigoNuevo(null)} />
        )}
        {pantalla === "sistema" && (
          <div className="h-full overflow-auto p-6">
            <Inicio />
          </div>
        )}
      </main>
    </div>
  );
}
