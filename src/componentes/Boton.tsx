import type { ButtonHTMLAttributes, ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import Tecla from "./Tecla";

export type VarianteBoton = "primario" | "secundario" | "terciario" | "peligro";
export type TamanoBoton = "normal" | "chico" | "grande" | "icono";

interface Props extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "className"> {
  /** Una sola acción primaria por pantalla o diálogo; lo opcional es terciario; lo que descarta, peligro. */
  variante?: VarianteBoton;
  /** normal 40 px, chico 32 px, grande 56 px (solo Cobrar), icono 32 px cuadrado (un ícono solo). */
  tamano?: TamanoBoton;
  icono?: LucideIcon;
  /** Atajo de teclado, a la derecha como una tecla. */
  atajo?: string;
  /** Ancho completo, con el contenido a la izquierda. */
  bloque?: boolean;
  /** Para el espaciado y el ancho desde fuera (márgenes, flex); el aspecto lo pone la variante. */
  className?: string;
  children?: ReactNode;
}

/** El único botón de la app: variante, tamaño, ícono y atajo. Sin animaciones, solo color. */
export default function Boton({
  variante = "secundario",
  tamano = "normal",
  icono: Icono,
  atajo,
  bloque,
  className,
  children,
  type = "button",
  ...resto
}: Props) {
  const clases = [
    "boton",
    `boton-${variante}`,
    tamano === "chico" && "boton-chico",
    tamano === "grande" && "boton-grande",
    tamano === "icono" && "boton-icono",
    bloque && "boton-bloque",
    className,
  ];
  return (
    <button type={type} className={clases.filter(Boolean).join(" ")} {...resto}>
      {Icono && <Icono size={tamano === "grande" ? 20 : 16} aria-hidden />}
      {children}
      {atajo && <Tecla>{atajo}</Tecla>}
    </button>
  );
}
