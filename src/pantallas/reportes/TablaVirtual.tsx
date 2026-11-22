import { useVirtualizer } from "@tanstack/react-virtual";
import { useRef, type ReactNode } from "react";

export interface ColumnaTabla<T> {
  titulo: string;
  /** Pista de grid de CSS: "minmax(0,1fr)", "7rem"… */
  ancho: string;
  derecha?: boolean;
  celda: (fila: T, i: number) => ReactNode;
}

const ALTO_FILA = 36;

/** Tabla con muchas filas: solo se dibujan las que se ven. */
export default function TablaVirtual<T>({
  filas,
  columnas,
  clave,
  vacia,
  alto = 420,
}: {
  filas: T[];
  columnas: ColumnaTabla<T>[];
  clave: (fila: T) => string | number;
  vacia: string;
  alto?: number;
}) {
  const contenedor = useRef<HTMLDivElement>(null);
  const virtual = useVirtualizer({
    count: filas.length,
    getScrollElement: () => contenedor.current,
    estimateSize: () => ALTO_FILA,
    overscan: 12,
  });
  const grid = { gridTemplateColumns: columnas.map((c) => c.ancho).join(" ") };
  return (
    <div className="overflow-hidden rounded-lg border border-linea">
      <div style={grid} className="grid gap-3 border-b border-linea bg-suave px-3 py-2 text-xs font-medium text-tenue">
        {columnas.map((c) => (
          <span key={c.titulo} className={c.derecha ? "text-right" : ""}>
            {c.titulo}
          </span>
        ))}
      </div>
      {filas.length === 0 ? (
        <p className="p-6 text-center text-tenue">{vacia}</p>
      ) : (
        <div ref={contenedor} className="overflow-auto" style={{ maxHeight: alto }}>
          <div style={{ height: virtual.getTotalSize(), position: "relative" }}>
            {virtual.getVirtualItems().map((v) => {
              const f = filas[v.index];
              return (
                <div
                  key={clave(f)}
                  style={{ ...grid, position: "absolute", top: 0, left: 0, width: "100%", height: ALTO_FILA, transform: `translateY(${v.start}px)` }}
                  className="grid items-center gap-3 border-b border-linea px-3 tabular-nums"
                >
                  {columnas.map((c) => (
                    <span key={c.titulo} className={`min-w-0 truncate ${c.derecha ? "text-right" : ""}`}>
                      {c.celda(f, v.index)}
                    </span>
                  ))}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
