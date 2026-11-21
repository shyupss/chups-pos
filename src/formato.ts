const clp = new Intl.NumberFormat("es-CL", {
  style: "currency",
  currency: "CLP",
  maximumFractionDigits: 0,
});

/** 1990 -> "$1.990" */
export const pesos = (monto: number) => clp.format(monto);

/** Deja solo dígitos: los montos son enteros, nunca decimales. */
export const soloDigitos = (texto: string) => texto.replace(/\D/g, "");

/** "1.990" -> 1990; vacío -> null */
export const aEntero = (texto: string): number | null => {
  const d = soloDigitos(texto);
  return d === "" ? null : Number(d);
};

export const folio = (n: number) => `N° ${String(n).padStart(6, "0")}`;
