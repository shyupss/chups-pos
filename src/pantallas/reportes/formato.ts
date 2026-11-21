import { pesos } from "../../formato";

const decimal = new Intl.NumberFormat("es-CL", { maximumFractionDigits: 1 });

/** Puntos básicos (1234 = 12,34 %) como porcentaje con un decimal: "12,3 %". */
export const porcentaje = (pb: number) => `${decimal.format(pb / 100)} %`;

/** Montos cortos para los ejes: $850, $12 mil, $1,2 M. */
export function pesosCortos(n: number): string {
  const abs = Math.abs(n);
  if (abs >= 1_000_000) return `$${decimal.format(n / 1_000_000)} M`;
  if (abs >= 10_000) return `$${decimal.format(Math.round(n / 1000))} mil`;
  return pesos(n);
}
