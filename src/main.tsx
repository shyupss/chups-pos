import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { instalarAtajos } from "./atajos";
import { iniciarTema } from "./hooks/useTema";
// Solo el subconjunto latin de Inter, empaquetado con la app.
import "./fuente-inter.css";
import "./index.css";

iniciarTema();
// Un solo listener de teclado para toda la app (src/atajos.ts).
instalarAtajos();

const TIPOS_DE_TEXTO = new Set(["text", "search", "tel", "number", "password", "email", "url"]);

/** Un campo donde se puede escribir: ahí sí sirve el menú de cortar, copiar y pegar. */
function esCampoEditable(el: EventTarget | null): boolean {
  if (el instanceof HTMLTextAreaElement) return !el.readOnly && !el.disabled;
  if (el instanceof HTMLInputElement) return TIPOS_DE_TEXTO.has(el.type) && !el.readOnly && !el.disabled;
  return false;
}

// El clic derecho no muestra el menú del navegador (Atrás, Actualizar, Imprimir…), salvo en los
// campos de texto, donde sirve para pegar la licencia o un RUT.
window.addEventListener("contextmenu", (e) => {
  if (!esCampoEditable(e.target)) e.preventDefault();
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
