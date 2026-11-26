import { useMemo, type ReactNode } from "react";
import Tabla from "../../componentes/Tabla";

export interface ColumnaTabla<T> {
  titulo: string;
  /** Pista de grid de CSS: "minmax(0,1fr)", "7rem"… */
  ancho: string;
  derecha?: boolean;
  celda: (fila: T, i: number) => ReactNode;
  /** Valor para ordenar al hacer clic en el encabezado; sin esto la columna no se ordena. */
  orden?: (fila: T) => string | number | null | undefined;
}

/**
 * La tabla común (src/componentes/Tabla.tsx) para los reportes y las vistas previas de importar:
 * una lista corta que crece hasta `alto` px y luego se desplaza. Solo se dibujan las filas que se ven.
 */
export default function TablaVirtual<T>({
  filas,
  columnas,
  clave,
  vacia,
  alto = 420,
  etiqueta = "Tabla",
}: {
  filas: T[];
  columnas: ColumnaTabla<T>[];
  clave: (fila: T) => string | number;
  vacia: string;
  alto?: number;
  etiqueta?: string;
}) {
  // Cada fila lleva su posición: algunas columnas la usan ("#1", "#2"…).
  const items = useMemo(() => filas.map((fila, i) => ({ fila, i })), [filas]);
  const columnasComunes = useMemo(
    () =>
      columnas.map((c) => ({
        id: c.titulo,
        titulo: c.titulo,
        ancho: c.ancho,
        alinear: c.derecha ? ("derecha" as const) : ("izquierda" as const),
        orden: c.orden ? (x: { fila: T; i: number }) => c.orden!(x.fila) : undefined,
        celda: (x: { fila: T; i: number }) => c.celda(x.fila, x.i),
      })),
    [columnas],
  );
  return (
    <Tabla
      etiqueta={etiqueta}
      alto={alto}
      columnas={columnasComunes}
      filas={items}
      clave={(x) => clave(x.fila)}
      vacio={{ texto: vacia }}
    />
  );
}
