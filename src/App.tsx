import { useCallback, useEffect, useState } from "react";
import BarraLateral, { type ModoBarra } from "./componentes/BarraLateral";
import Caja from "./pantallas/Caja";
import Configuracion from "./pantallas/Configuracion";
import Productos from "./pantallas/Productos";
import Proximamente from "./pantallas/Proximamente";
import { guardarPreferencia, leerPreferencia } from "./preferencias";
import { SECCIONES, type Seccion } from "./secciones";

const CLAVE_BARRA = "chups-pos.barra-lateral";

export default function App() {
  const [seccion, setSeccion] = useState<Seccion>("caja");
  // Código escaneado en la caja que no existe: se abre Productos con el formulario listo.
  const [codigoNuevo, setCodigoNuevo] = useState<string | null>(null);
  // Ventas en espera: la Caja las informa y la barra lateral las muestra en su ítem.
  const [enEspera, setEnEspera] = useState(0);
  const [modoBarra, setModoBarra] = useState<ModoBarra>(() =>
    leerPreferencia<ModoBarra>(CLAVE_BARRA, ["auto", "expandida", "colapsada"], "auto"),
  );

  // Atajos globales F1–F6 y F9. Con modificadores no se tocan (Alt+F4 debe cerrar la ventana).
  useEffect(() => {
    const alPresionar = (e: KeyboardEvent) => {
      if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
      const s = SECCIONES.find((x) => x.tecla === e.key);
      if (s) {
        e.preventDefault();
        setSeccion(s.id);
      }
    };
    window.addEventListener("keydown", alPresionar);
    return () => window.removeEventListener("keydown", alPresionar);
  }, []);

  const crearDesdeCaja = useCallback((codigo: string) => {
    setCodigoNuevo(codigo);
    setSeccion("productos");
  }, []);

  const cambiarModoBarra = useCallback((modo: ModoBarra) => {
    setModoBarra(modo);
    guardarPreferencia(CLAVE_BARRA, modo);
  }, []);

  const expandida = modoBarra === "expandida" || (modoBarra === "auto" && seccion !== "caja");
  const pendiente = SECCIONES.find((s) => s.id === seccion && s.pendiente);

  return (
    <div className="flex h-full">
      <BarraLateral
        actual={seccion}
        expandida={expandida}
        onIr={setSeccion}
        onAlternar={() => cambiarModoBarra(expandida ? "colapsada" : "expandida")}
        insignias={
          enEspera > 0
            ? { caja: { valor: enEspera, descripcion: enEspera === 1 ? "1 venta en espera" : `${enEspera} ventas en espera` } }
            : {}
        }
      />
      <main className="min-w-0 flex-1">
        {/* Caja queda montada siempre: cambiar de sección no borra el carrito. */}
        <div className={seccion === "caja" ? "h-full" : "hidden"}>
          <Caja activa={seccion === "caja"} onCrearProducto={crearDesdeCaja} onEnEspera={setEnEspera} />
        </div>
        {seccion === "productos" && (
          <Productos codigoNuevo={codigoNuevo} onCodigoUsado={() => setCodigoNuevo(null)} />
        )}
        {seccion === "configuracion" && <Configuracion modoBarra={modoBarra} onModoBarra={cambiarModoBarra} />}
        {pendiente && <Proximamente seccion={pendiente} />}
      </main>
    </div>
  );
}
