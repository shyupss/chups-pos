import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { iniciarTema } from "./hooks/useTema";
// Solo el subconjunto latin de Inter, empaquetado con la app.
import "./fuente-inter.css";
import "./index.css";

iniciarTema();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
