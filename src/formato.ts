import type { Unidad } from "./api";

const clp = new Intl.NumberFormat("es-CL", {
  style: "currency",
  currency: "CLP",
  maximumFractionDigits: 0,
});
const enteros = new Intl.NumberFormat("es-CL", { maximumFractionDigits: 0 });

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

// Cantidades en milésimas (enteros): 1 unidad = 1000, 0,532 kg = 532.
// Los decimales solo existen al mostrar o al leer lo que escribe el usuario.

/** 1532 -> "1,532"; -532 -> "-0,532". Parte entera y resto por separado, sin aritmética decimal. */
function conTresDecimales(mil: number, separarMiles: boolean): string {
  const signo = mil < 0 ? "-" : "";
  const abs = Math.abs(mil);
  const entera = Math.floor(abs / 1000);
  const resto = String(abs % 1000).padStart(3, "0");
  return `${signo}${separarMiles ? enteros.format(entera) : entera},${resto}`;
}

/** (3000, "unidad") -> "3"; (532, "kg") -> "0,532 kg". */
export function cantidad(mil: number, unidad: Unidad): string {
  if (unidad === "unidad" && mil % 1000 === 0) return enteros.format(mil / 1000);
  const texto = conTresDecimales(mil, true);
  return unidad === "kg" ? `${texto} kg` : texto;
}

/** Valor para un campo editable: (5000, "unidad") -> "5"; (1500, "kg") -> "1,500". */
export function cantidadEditable(mil: number, unidad: Unidad): string {
  if (unidad === "unidad" && mil % 1000 === 0) return String(mil / 1000);
  return conTresDecimales(mil, false);
}

/**
 * Peso escrito por el usuario, en milésimas de kg (gramos).
 * Acepta kg ("0,5", "0.532", ",5", "1,25 kg") o gramos ("532g", "532 g", "532 gr").
 * Devuelve null si el texto no es un peso válido; no revisa límites.
 */
export function parseKg(texto: string): number | null {
  const t = texto.trim().toLowerCase();
  const gramos = t.match(/^(\d{1,6})\s*(?:g|gr|grs)$/);
  if (gramos) return Number(gramos[1]);
  const kg = t.match(/^(\d{0,6})(?:[.,](\d{1,3}))?\s*(?:kg|kgs|kilo|kilos)?$/);
  if (!kg || (kg[1] === "" && kg[2] === undefined)) return null;
  return Number(kg[1] || "0") * 1000 + Number((kg[2] ?? "").padEnd(3, "0"));
}

/** Cantidad escrita en un campo de stock: unidades enteras o kg según el producto. */
export function parseCantidad(texto: string, unidad: Unidad): number | null {
  if (unidad === "kg") return parseKg(texto);
  const t = texto.trim();
  return /^\d{1,9}$/.test(t) ? Number(t) * 1000 : null;
}

/**
 * Subtotal mostrado de una línea: (precio × cantidadMil + 500) / 1000, redondeado al peso.
 * Es la misma regla que aplica Rust; el valor que vale es el que devuelve al confirmar.
 */
export const subtotalLinea = (precio: number, cantidadMil: number) =>
  Math.floor((precio * cantidadMil + 500) / 1000);
