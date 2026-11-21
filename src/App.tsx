import { useCallback, useEffect, useState } from "react";
import { api, type EstadoCaja, type EstadoConfigurada } from "./api";
import BarraLateral, { type ModoBarra } from "./componentes/BarraLateral";
import Caja from "./pantallas/Caja";
import { ProveedorPin } from "./componentes/PedirPin";
import { escucharCambioStock } from "./eventos";
import Configuracion from "./pantallas/Configuracion";
import Inventario, { type PestanaInventario } from "./pantallas/Inventario";
import Productos from "./pantallas/Productos";
import Proximamente from "./pantallas/Proximamente";
import Ventas from "./pantallas/Ventas";
import { guardarPreferencia, leerPreferencia } from "./preferencias";
import { SECCIONES, type Seccion } from "./secciones";

const CLAVE_BARRA = "chups-pos.barra-lateral";

export default function App() {
  const [seccion, setSeccion] = useState<Seccion>("caja");
  // Código escaneado en la caja que no existe: se abre Productos con el formulario listo.
  const [codigoNuevo, setCodigoNuevo] = useState<string | null>(null);
  // Ventas en espera: la Caja las informa y la barra lateral las muestra en su ítem.
  const [enEspera, setEnEspera] = useState(0);
  // Estado de la impresora para la barra lateral: al abrir y cada 60 s, nunca en cada render.
  const [impresora, setImpresora] = useState<EstadoConfigurada | null>(null);
  const refrescarImpresora = useCallback(() => {
    api
      .estadoImpresora()
      .then(setImpresora)
      .catch(() => {});
  }, []);
  useEffect(() => {
    refrescarImpresora();
    const intervalo = setInterval(refrescarImpresora, 60_000);
    return () => clearInterval(intervalo);
  }, [refrescarImpresora]);
  // Turno de caja: la Caja muestra "Abrir caja" sin turno y la barra lateral dice desde cuándo está abierta.
  const [caja, setCaja] = useState<EstadoCaja | null>(null);
  const refrescarCaja = useCallback(() => {
    api
      .estadoCaja()
      .then(setCaja)
      .catch(() => {});
  }, []);
  useEffect(() => {
    refrescarCaja();
    // También pasa la medianoche: "desde 09:12" debe pasar a decir el día.
    const intervalo = setInterval(refrescarCaja, 60_000);
    return () => clearInterval(intervalo);
  }, [refrescarCaja]);
  // Inventario: pestaña, producto filtrado en Movimientos y la insignia de productos bajo el mínimo,
  // que se vuelve a contar cuando alguien avisa que cambió el stock (sin sondeo).
  const [pestanaInventario, setPestanaInventario] = useState<PestanaInventario>("stock");
  const [productoMovimientos, setProductoMovimientos] = useState<{ id: number; nombre: string } | null>(null);
  const [bajoMinimo, setBajoMinimo] = useState(0);
  useEffect(() => {
    const contar = () => {
      api
        .contarBajoMinimo()
        .then(setBajoMinimo)
        .catch(() => {});
    };
    contar();
    return escucharCambioStock(contar);
  }, []);
  // Venta que se abre al llegar a Ventas (enlace desde un movimiento de stock).
  const [folioVentas, setFolioVentas] = useState<number | null>(null);
  const verMovimientos = useCallback((p: { id: number; nombre: string }) => {
    setProductoMovimientos(p);
    setPestanaInventario("movimientos");
    setSeccion("inventario");
  }, []);
  const irAVenta = useCallback((folio: number) => {
    setFolioVentas(folio);
    setSeccion("ventas");
  }, []);
  const [modoBarra, setModoBarra] = useState<ModoBarra>(() =>
    leerPreferencia<ModoBarra>(CLAVE_BARRA, ["auto", "expandida", "colapsada"], "auto"),
  );

  // Atajos globales F1–F6 y F9. Con modificadores no se tocan (Alt+F4 debe cerrar la ventana).
  useEffect(() => {
    const alPresionar = (e: KeyboardEvent) => {
      // Ctrl+P no abre el diálogo de impresión del navegador (en la Caja reimprime la boleta).
      if (e.ctrlKey && e.key.toLowerCase() === "p") e.preventDefault();
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
    <ProveedorPin>
      <div className="flex h-full">
        <BarraLateral
          actual={seccion}
          expandida={expandida}
          onIr={setSeccion}
          onAlternar={() => cambiarModoBarra(expandida ? "colapsada" : "expandida")}
          impresora={impresora}
          turno={caja === null ? undefined : caja.turno}
          insignias={{
            ...(enEspera > 0
              ? { caja: { valor: enEspera, descripcion: enEspera === 1 ? "1 venta en espera" : `${enEspera} ventas en espera` } }
              : {}),
            ...(bajoMinimo > 0
              ? {
                  inventario: {
                    valor: bajoMinimo,
                    descripcion: bajoMinimo === 1 ? "1 producto bajo el mínimo" : `${bajoMinimo} productos bajo el mínimo`,
                  },
                }
              : {}),
          }}
        />
        <main className="min-w-0 flex-1">
          {/* Caja queda montada siempre: cambiar de sección no borra el carrito. */}
          <div className={seccion === "caja" ? "h-full" : "hidden"}>
            <Caja
              activa={seccion === "caja"}
              onCrearProducto={crearDesdeCaja}
              onEnEspera={setEnEspera}
              onImpresion={refrescarImpresora}
              caja={caja}
              onCambioCaja={refrescarCaja}
            />
          </div>
          {seccion === "productos" && (
            <Productos codigoNuevo={codigoNuevo} onCodigoUsado={() => setCodigoNuevo(null)} onVerMovimientos={verMovimientos} />
          )}
          {seccion === "inventario" && (
            <Inventario
              activa
              pestana={pestanaInventario}
              onPestana={setPestanaInventario}
              producto={productoMovimientos}
              onProducto={setProductoMovimientos}
              onIrAVenta={irAVenta}
            />
          )}
          {seccion === "ventas" && (
            <Ventas
              activa
              onImpresion={refrescarImpresora}
              folioInicial={folioVentas}
              onFolioUsado={() => setFolioVentas(null)}
            />
          )}
          {seccion === "configuracion" && (
            <Configuracion
              modoBarra={modoBarra}
              onModoBarra={cambiarModoBarra}
              onCambioImpresora={refrescarImpresora}
            />
          )}
          {pendiente && <Proximamente seccion={pendiente} />}
        </main>
      </div>
    </ProveedorPin>
  );
}
