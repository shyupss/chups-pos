import type { ReactNode } from "react";

/** Una tecla de atajo (Enter, F12, Ctrl P…). Dentro del botón primario se ve translúcida. */
export default function Tecla({ children }: { children: ReactNode }) {
  return <kbd className="tecla">{children}</kbd>;
}
