import Boton from "./componentes/Boton";
import { useCallback, useEffect, useRef, useState } from "react";
import { useAtajos } from "./atajos";
import AyudaAtajos from "./componentes/AyudaAtajos";
import {
  api,
  escucharActualizacion,
  escucharRespaldoFallido,
  textoError,
  type EstadoActualizacion,
  type EstadoCaja,
  type EstadoConfigurada,
  type EstadoLicencia,
} from "./api";
import Dialogo from "./componentes/Dialogo";
import AvisoIntegridad from "./componentes/AvisoIntegridad";
import AvisoLicencia from "./componentes/AvisoLicencia";
import BarraLateral, { type ModoBarra } from "./componentes/BarraLateral";
import Caja from "./pantallas/Caja";
import { ProveedorPin } from "./componentes/PedirPin";
import { escucharCambioStock } from "./eventos";
import Asistente, { type DestinoAsistente } from "./pantallas/Asistente";
import Configuracion, { irASeccionConfig } from "./pantallas/Configuracion";
import Inventario, { type PestanaInventario } from "./pantallas/Inventario";
import Productos from "./pantallas/Productos";
import Fiados from "./pantallas/Fiados";
import Proximamente from "./pantallas/Proximamente";
import Reportes, { type PestanaReportes } from "./pantallas/Reportes";
import type { FiltroEstadoStock } from "./api";
import Ventas from "./pantallas/Ventas";
import { guardarPreferencia, leerPreferencia } from "./preferencias";
import { pedirSalida } from "./salida";
import { SECCIONES, type Seccion } from "./secciones";

const CLAVE_BARRA = "chups-pos.barra-lateral";

export default function App() {
  const [seccion, setSeccionDirecto] = useState<Seccion>("caja");
  const seccionActual = useRef(seccion);
  seccionActual.current = seccion;
  // Cambiar de sección pregunta antes si hay cambios sin guardar (Configuración > Negocio).
  const setSeccion = useCallback((s: Seccion) => {
    if (s === seccionActual.current) return;
    pedirSalida(() => setSeccionDirecto(s));
  }, []);
  const [ayudaAtajos, setAyudaAtajos] = useState(false);
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
  // Licencia: el estado lo decide Rust; aquí solo se muestra (barra lateral, aviso y Abrir caja).
  // Se relee cada minuto, porque los días de prueba cambian solos.
  const [licencia, setLicencia] = useState<EstadoLicencia | null>(null);
  const refrescarLicencia = useCallback(() => {
    api
      .estadoLicencia()
      .then(setLicencia)
      .catch(() => {});
  }, []);
  useEffect(() => {
    refrescarLicencia();
    const intervalo = setInterval(refrescarLicencia, 60_000);
    return () => clearInterval(intervalo);
  }, [refrescarLicencia]);
  // Actualizaciones: Rust las busca solo (una vez al día, sin turno abierto) y avisa con un evento.
  const [actualizacion, setActualizacion] = useState<EstadoActualizacion | null>(null);
  const refrescarActualizacion = useCallback(() => {
    api
      .estadoActualizacion()
      .then(setActualizacion)
      .catch(() => {});
  }, []);
  useEffect(() => {
    refrescarActualizacion();
    return escucharActualizacion(refrescarActualizacion);
  }, [refrescarActualizacion]);
  const [instalando, setInstalando] = useState(false);
  const [errorInstalacion, setErrorInstalacion] = useState<string | null>(null);
  // "Instalar al cerrar el turno": cuando el turno pasa de abierto a cerrado y quedó pedido, se instala.
  const hayTurno = caja?.turno != null;
  const habiaTurno = useRef(false);
  useEffect(() => {
    if (habiaTurno.current && !hayTurno && actualizacion?.instalarAlCerrar) {
      setInstalando(true);
      api.instalarActualizacion().catch((e) => {
        setInstalando(false);
        setErrorInstalacion(textoError(e));
      });
    }
    habiaTurno.current = hayTurno;
  }, [hayTurno, actualizacion?.instalarAlCerrar]);
  // Asistente de primer arranque: null mientras se pregunta a Rust; true con la base vacía (o si se
  // abre a mano desde Configuración); false cuando ya no hace falta.
  const [asistente, setAsistente] = useState<boolean | null>(null);
  const [asistenteReabierto, setAsistenteReabierto] = useState(false);
  useEffect(() => {
    api
      .asistentePendiente()
      .then(setAsistente)
      .catch(() => setAsistente(false));
  }, []);
  // Abrir Configuración en Acerca de (desde el aviso de actualización de la barra lateral).
  const irAActualizaciones = useCallback(() => {
    pedirSalida(() => {
      irASeccionConfig("acerca");
      setClaveConfig((k) => k + 1);
      setSeccionDirecto("configuracion");
    });
  }, []);
  // Abrir Configuración en Licencia (desde el aviso, la barra o Abrir caja). La clave la vuelve a
  // montar si ya estaba abierta en otra sección.
  const [claveConfig, setClaveConfig] = useState(0);
  const irALicencia = useCallback(() => {
    pedirSalida(() => {
      irASeccionConfig("licencia");
      setClaveConfig((k) => k + 1);
      setSeccionDirecto("configuracion");
    });
  }, []);
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
  // Un respaldo automático que falló: se avisa sin interrumpir la caja.
  const [respaldoFallido, setRespaldoFallido] = useState<string | null>(null);
  useEffect(() => escucharRespaldoFallido(setRespaldoFallido), []);
  // Enlaces desde Reportes > Hoy: Inventario filtrado bajo el mínimo y la ficha de un cliente.
  const [pestanaReportes, setPestanaReportes] = useState<PestanaReportes>("hoy");
  const [estadoStock, setEstadoStock] = useState<FiltroEstadoStock | null>(null);
  const [clienteFiados, setClienteFiados] = useState<number | null>(null);
  useEffect(() => {
    if (seccion !== "inventario") setEstadoStock(null);
    if (seccion !== "fiados") setClienteFiados(null);
  }, [seccion]);
  const verBajoMinimo = useCallback(() => {
    setEstadoStock("bajoMinimo");
    setPestanaInventario("stock");
    setSeccion("inventario");
  }, [setSeccion]);
  const verCliente = useCallback(
    (id: number) => {
      setClienteFiados(id);
      setSeccion("fiados");
    },
    [setSeccion],
  );
  // Venta que se abre al llegar a Ventas (enlace desde un movimiento de stock).
  const [folioVentas, setFolioVentas] = useState<number | null>(null);
  const verMovimientos = useCallback(
    (p: { id: number; nombre: string }) => {
      setProductoMovimientos(p);
      setPestanaInventario("movimientos");
      setSeccion("inventario");
    },
    [setSeccion],
  );
  const irAVenta = useCallback(
    (folio: number) => {
      setFolioVentas(folio);
      setSeccion("ventas");
    },
    [setSeccion],
  );
  const [modoBarra, setModoBarra] = useState<ModoBarra>(() =>
    leerPreferencia<ModoBarra>(CLAVE_BARRA, ["auto", "expandida", "colapsada"], "auto"),
  );

  // Atajos globales F1–F6 y F9, solo sin modificadores (Alt+F4 debe cerrar la ventana). Con un
  // diálogo abierto no corren: lo decide el despachador (src/atajos.ts).
  useAtajos(
    "global",
    Object.fromEntries(
      SECCIONES.map((s) => [
        s.tecla,
        () =>
          // F6 siempre abre Reportes en Hoy.
          s.id === "reportes"
            ? pedirSalida(() => {
                setPestanaReportes("hoy");
                setSeccionDirecto("reportes");
              })
            : setSeccion(s.id),
      ]),
    ),
    asistente === false,
  );

  const crearDesdeCaja = useCallback(
    (codigo: string) => {
      setCodigoNuevo(codigo);
      setSeccion("productos");
    },
    [setSeccion],
  );

  const cambiarModoBarra = useCallback((modo: ModoBarra) => {
    setModoBarra(modo);
    guardarPreferencia(CLAVE_BARRA, modo);
  }, []);

  const terminarAsistente = useCallback(
    (destino: DestinoAsistente) => {
      setAsistente(false);
      setAsistenteReabierto(false);
      refrescarImpresora();
      refrescarCaja();
      refrescarLicencia();
      if (destino === "producto-nuevo") {
        setCodigoNuevo("");
        setSeccionDirecto("productos");
      } else {
        setSeccionDirecto(destino === "productos" ? "productos" : "caja");
      }
    },
    [refrescarImpresora, refrescarCaja, refrescarLicencia],
  );

  const expandida = modoBarra === "expandida" || (modoBarra === "auto" && seccion !== "caja");
  const pendiente = SECCIONES.find((s) => s.id === seccion && s.pendiente);

  // Al abrir con la base vacía, el asistente ocupa el lugar de la Caja (y de todo lo demás).
  if (asistente === null) return <div className="h-full bg-fondo" />;
  if (asistente) {
    return (
      <ProveedorPin>
        <Asistente reabierto={asistenteReabierto} onCambioImpresora={refrescarImpresora} onTerminar={terminarAsistente} />
      </ProveedorPin>
    );
  }

  return (
    <ProveedorPin>
      <AvisoIntegridad />
      <div className="flex h-full">
        <BarraLateral
          actual={seccion}
          expandida={expandida}
          onIr={setSeccion}
          onAlternar={() => cambiarModoBarra(expandida ? "colapsada" : "expandida")}
          impresora={impresora}
          turno={caja === null ? undefined : caja.turno}
          licencia={licencia}
          onLicencia={irALicencia}
          actualizacion={actualizacion}
          onActualizacion={irAActualizaciones}
          onAtajos={() => setAyudaAtajos(true)}
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
        <main className="flex min-w-0 flex-1 flex-col">
          <AvisoLicencia estado={licencia} onActivar={irALicencia} />
          <div className="min-h-0 flex-1">
          {/* Caja queda montada siempre: cambiar de sección no borra el carrito. */}
          <div className={seccion === "caja" ? "h-full" : "hidden"}>
            <Caja
              activa={seccion === "caja"}
              onCrearProducto={crearDesdeCaja}
              onEnEspera={setEnEspera}
              onImpresion={refrescarImpresora}
              caja={caja}
              onCambioCaja={refrescarCaja}
              puedeVender={licencia?.puedeVender ?? true}
              onIrALicencia={irALicencia}
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
              estadoStock={estadoStock}
            />
          )}
          {seccion === "fiados" && (
            <Fiados activa onIrAVenta={irAVenta} onImpresion={refrescarImpresora} clienteInicial={clienteFiados} />
          )}
          {seccion === "reportes" && (
            <Reportes
              activa
              pestana={pestanaReportes}
              onPestana={setPestanaReportes}
              onVerBajoMinimo={verBajoMinimo}
              onVerCliente={verCliente}
              onVerProducto={verMovimientos}
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
              key={claveConfig}
              modoBarra={modoBarra}
              onModoBarra={cambiarModoBarra}
              onCambioImpresora={refrescarImpresora}
              licencia={licencia}
              onCambioLicencia={setLicencia}
              onAbrirAsistente={() =>
                pedirSalida(() => {
                  setAsistenteReabierto(true);
                  setAsistente(true);
                })
              }
              actualizacion={actualizacion}
              onActualizacion={setActualizacion}
              hayTurno={hayTurno}
              onInstalando={() => setInstalando(true)}
            />
          )}
          {pendiente && <Proximamente seccion={pendiente} />}
          </div>
        </main>
        {ayudaAtajos && <AyudaAtajos onCerrar={() => setAyudaAtajos(false)} />}
        {instalando && (
          <Dialogo titulo="Instalando la actualización" onCerrar={() => {}}>
            <p role="alert" className="mt-2 text-tenue">
              Se hizo un respaldo de tus datos. CHUPS POS se cerrará y volverá a abrirse en unos instantes: no apagues el
              computador.
            </p>
          </Dialogo>
        )}
        {errorInstalacion && (
          <div role="alert" className="fixed right-4 bottom-4 z-30 max-w-md tarjeta p-4 shadow-s3">
            <p className="font-medium text-peligro">{errorInstalacion}</p>
            <div className="mt-3 flex justify-end">
              <Boton variante="terciario" tamano="chico" onClick={() => setErrorInstalacion(null)}>
                Cerrar
              </Boton>
            </div>
          </div>
        )}
        {respaldoFallido && (
          <div role="alert" className="fixed right-4 bottom-4 z-30 max-w-md tarjeta p-4 shadow-s3">
            <p className="font-medium text-peligro">{respaldoFallido}</p>
            <div className="mt-3 flex justify-end gap-2 text-sm">
              <Boton variante="terciario" tamano="chico" onClick={() => setRespaldoFallido(null)}>
                Cerrar
              </Boton>
              <Boton
                tamano="chico"
                onClick={() => {
                  setRespaldoFallido(null);
                  setSeccion("configuracion");
                }}
              >
                Ver respaldos
              </Boton>
            </div>
          </div>
        )}
      </div>
    </ProveedorPin>
  );
}
