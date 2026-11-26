import type { MedioPago, Unidad } from "./api";

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

/** Máximo por línea, en milésimas: 100 kg o 100.000 unidades. */
export const MAX_KG_MIL = 100_000;
export const MAX_UNIDADES_MIL = 100_000_000;

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

/**
 * Ajuste del efectivo a la decena (1 a 5 baja, 6 a 9 sube): 1224 -> -4, 1226 -> +4.
 * Solo para mostrarlo antes de cobrar; el redondeo que vale lo aplica Rust al confirmar.
 */
export function redondeoEfectivo(monto: number): number {
  const resto = ((monto % 10) + 10) % 10;
  return resto <= 5 ? -resto : 10 - resto;
}

const reloj = new Intl.DateTimeFormat("es-CL", { hour: "2-digit", minute: "2-digit", hourCycle: "h23" });

/** Fecha ISO en UTC (como la guarda la base) -> "14:05" en la hora local del PC. */
export const hora = (iso: string) => reloj.format(new Date(iso));

/** Fecha local "aaaa-mm-dd" (la de los filtros por día). */
export function diaLocal(fecha: Date): string {
  const dos = (n: number) => String(n).padStart(2, "0");
  return `${fecha.getFullYear()}-${dos(fecha.getMonth() + 1)}-${dos(fecha.getDate())}`;
}

/** La carpeta de una ruta de Windows o de cualquier otra: todo lo que va antes del último separador. */
export function carpetaDe(ruta: string): string {
  const i = Math.max(ruta.lastIndexOf("\\"), ruta.lastIndexOf("/"));
  return i > 0 ? ruta.slice(0, i) : ruta;
}

/** "21-11-2026 14:05" en la hora local, igual que en la boleta. */
export function fechaHora(iso: string): string {
  const [a, m, d] = diaLocal(new Date(iso)).split("-");
  return `${d}-${m}-${a} ${hora(iso)}`;
}

const diaLargo = new Intl.DateTimeFormat("es-CL", { weekday: "long", day: "numeric", month: "long" });
const diaLargoConAnio = new Intl.DateTimeFormat("es-CL", { weekday: "long", day: "numeric", month: "long", year: "numeric" });

/** "Hoy, sábado 21 de noviembre", "Ayer, viernes 20 de noviembre" o "jueves 19 de noviembre" (con año si no es el actual). */
export function etiquetaDia(iso: string): string {
  const fecha = new Date(iso);
  const hoy = new Date();
  const dia = diaLocal(fecha);
  const texto = (fecha.getFullYear() === hoy.getFullYear() ? diaLargo : diaLargoConAnio).format(fecha);
  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
  if (dia === diaLocal(hoy)) return `Hoy, ${texto}`;
  if (dia === diaLocal(new Date(hoy.getFullYear(), hoy.getMonth(), hoy.getDate() - 1))) return `Ayer, ${texto}`;
  return cap(texto);
}

/** "09:12" si fue hoy; "20-11 09:12" si fue otro día. */
export function horaODia(iso: string): string {
  const dia = diaLocal(new Date(iso));
  if (dia === diaLocal(new Date())) return hora(iso);
  const [, m, d] = dia.split("-");
  return `${d}-${m} ${hora(iso)}`;
}

/** +$100 / −$100 / $0: diferencias y ajustes con su signo. */
export const pesosConSigno = (monto: number) =>
  monto > 0 ? `+${pesos(monto)}` : monto < 0 ? `−${pesos(-monto)}` : pesos(0);

/** Nombre para mostrar de un medio de pago. */
export const NOMBRE_MEDIO: Record<MedioPago, string> = {
  efectivo: "Efectivo",
  debito: "Débito",
  credito: "Crédito",
  transferencia: "Transferencia",
  fiado: "Fiado",
};
