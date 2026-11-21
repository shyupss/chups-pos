import Tecla from "../componentes/Tecla";
import Boton from "../componentes/Boton";
import { useCallback, useEffect, useRef, useState } from "react";
import { ArrowDownUp, Clock, Lock, Pause, Trash2 } from "lucide-react";
import {
  api,
  textoError,
  type Config,
  type EstadoCaja,
  type ItemPedido,
  type Producto,
  type ResumenEspera,
  type VentaConfirmada,
} from "../api";
import AbrirCaja from "../componentes/AbrirCaja";
import AvisoVenta from "../componentes/AvisoVenta";
import CerrarCaja from "../componentes/CerrarCaja";
import Cobro from "../componentes/Cobro";
import DialogoMovimiento from "../componentes/DialogoMovimiento";
import DialogoPeso from "../componentes/DialogoPeso";
import ListaEspera from "../componentes/ListaEspera";
import PonerEnEspera from "../componentes/PonerEnEspera";
import { cantidad, MAX_KG_MIL, MAX_UNIDADES_MIL, parseKg, pesos, subtotalLinea } from "../formato";
import { avisarCambioStock } from "../eventos";
import { useSalidasVenta } from "../hooks/useSalidasVenta";

interface Linea {
  producto: Producto;
  /** En milésimas: 3 unidades = 3000, 0,532 kg = 532. */
  cantidadMil: number;
}

interface Props {
  activa: boolean;
  onCrearProducto: (codigo: string) => void;
  /** Cuántas ventas hay en espera, para la insignia de la barra lateral. */
  onEnEspera: (cantidad: number) => void;
  /** Se intentó imprimir: conviene volver a leer el estado de la impresora. */
  onImpresion: () => void;
  /** Turno de caja; null mientras se lee. Sin turno abierto se muestra "Abrir caja". */
  caja: EstadoCaja | null;
  /** Se abrió o cerró la caja: App vuelve a leer el turno. */
  onCambioCaja: () => void;
}

/** Igual que el servicio: más ventas en espera no se aceptan. */
const MAXIMO_EN_ESPERA = 20;

type Aviso =
  | { tipo: "error"; texto: string }
  | { tipo: "no-existe"; codigo: string }
  | { tipo: "venta"; venta: VentaConfirmada }
  | { tipo: "info"; texto: string; alerta: boolean };

/** Lo que va antes del asterisco: "3" en "3*código", "0,5" en "0,5*pan". */
interface Multiplo {
  texto: string;
  decimal: boolean;
}

/** Diálogo de peso abierto: para agregar el producto o para cambiar el peso de su línea. */
interface Pesando {
  producto: Producto;
  cambiar: boolean;
}

const PARECE_CODIGO = /^[A-Za-z0-9-]+$/;
const maximoMil = (p: Producto) => (p.unidad === "kg" ? MAX_KG_MIL : MAX_UNIDADES_MIL);

/** "3*7801234" -> 3 del código 7801234; "0,5*pan" o "0.5x pan" -> 0,5 kg de pan. */
function separarCantidad(entrada: string): { multiplo: Multiplo | null; texto: string } {
  const m = entrada.match(/^(\d{1,6}(?:[.,]\d{1,3})?)\s*[*xX]\s*(.+)$/);
  if (m) return { multiplo: { texto: m[1], decimal: /[.,]/.test(m[1]) }, texto: m[2].trim() };
  return { multiplo: null, texto: entrada.trim() };
}

/** Cuánto agregar de un producto: milésimas, pedir el peso o un error para mostrar. */
function resolverCantidad(
  p: Producto,
  multiplo: Multiplo | null,
): { tipo: "mil"; mil: number } | { tipo: "pesar" } | { tipo: "error"; texto: string } {
  if (!multiplo) return p.unidad === "kg" ? { tipo: "pesar" } : { tipo: "mil", mil: 1000 };
  if (p.unidad === "unidad") {
    if (multiplo.decimal) {
      return { tipo: "error", texto: `${p.nombre} se vende por unidad: usa un número entero, como 3*código` };
    }
    const unidades = Number(multiplo.texto);
    return unidades >= 1
      ? { tipo: "mil", mil: Math.min(unidades * 1000, MAX_UNIDADES_MIL) }
      : { tipo: "error", texto: "La cantidad debe ser al menos 1" };
  }
  // Por kg, "2*pan" son 2 kg y "0,5*pan" medio kilo.
  const mil = parseKg(multiplo.texto);
  return mil !== null && mil >= 1 && mil <= MAX_KG_MIL
    ? { tipo: "mil", mil }
    : { tipo: "error", texto: `El peso de ${p.nombre} debe estar entre 1 g y 100 kg` };
}

export default function Caja({ activa, onCrearProducto, onEnEspera, onImpresion, caja, onCambioCaja }: Props) {
  const [lineas, setLineas] = useState<Linea[]>([]);
  const [entrada, setEntrada] = useState("");
  const [resultados, setResultados] = useState<Producto[]>([]);
  const [multiploPendiente, setMultiploPendiente] = useState<Multiplo | null>(null);
  const [aviso, setAviso] = useState<Aviso | null>(null);
  const [cobrando, setCobrando] = useState(false);
  const [pesando, setPesando] = useState<Pesando | null>(null);
  const [config, setConfig] = useState<Config | null>(null);
  const [ultimaVentaId, setUltimaVentaId] = useState<number | null>(null);
  // Producto agregado o cambiado en último lugar: va resaltado en la tabla y en el visor.
  const [ultimoId, setUltimoId] = useState<number | null>(null);
  const { salida, iniciar, imprimir, reintentar, guardarPdf, abrirCajon, cerrar, descartarSiTermino } =
    useSalidasVenta(onImpresion);
  const [dialogo, setDialogo] = useState<"poner-espera" | "lista-espera" | "movimiento" | "cerrar-caja" | null>(null);
  const [enEspera, setEnEspera] = useState<ResumenEspera[] | null>(null);
  const campo = useRef<HTMLInputElement>(null);

  const turno = caja?.turno ?? null;
  const cerrada = caja !== null && turno === null;
  // Con la caja cerrada se muestra "Abrir caja": los atajos de la Caja no deben actuar.
  const dialogoAbierto = cobrando || pesando !== null || dialogo !== null || cerrada;
  const enfocar = useCallback(() => campo.current?.focus(), []);

  useEffect(() => {
    if (activa && !dialogoAbierto) enfocar();
  }, [activa, dialogoAbierto, enfocar]);

  // Se vuelve a leer al entrar a la Caja, por si cambió en Configuración.
  useEffect(() => {
    if (!activa) return;
    api
      .obtenerConfig()
      .then(setConfig)
      .catch(() => {});
  }, [activa]);

  // Para Ctrl+P aunque la app recién se haya abierto.
  useEffect(() => {
    api
      .ultimaVenta()
      .then(setUltimaVentaId)
      .catch(() => {});
  }, []);

  const agregar = useCallback((producto: Producto, mil: number) => {
    setUltimoId(producto.id);
    setLineas((ls) => {
      const i = ls.findIndex((l) => l.producto.id === producto.id);
      if (i === -1) return [...ls, { producto, cantidadMil: Math.min(maximoMil(producto), mil) }];
      const copia = ls.slice();
      copia[i] = { producto, cantidadMil: Math.min(maximoMil(producto), copia[i].cantidadMil + mil) };
      return copia;
    });
    setResultados([]);
    setAviso(null);
    descartarSiTermino();
  }, [descartarSiTermino]);

  /** Agrega lo escaneado o elegido; los productos por kg sin cantidad abren el diálogo de peso. */
  const elegir = useCallback(
    (producto: Producto, multiplo: Multiplo | null) => {
      const r = resolverCantidad(producto, multiplo);
      if (r.tipo === "mil") return agregar(producto, r.mil);
      setResultados([]);
      if (r.tipo === "pesar") {
        setAviso(null);
        setPesando({ producto, cambiar: false });
      } else {
        setAviso({ tipo: "error", texto: r.texto });
      }
    },
    [agregar],
  );

  const procesarEntrada = useCallback(async () => {
    const { multiplo, texto } = separarCantidad(entrada);
    setEntrada("");
    if (!texto) return;
    try {
      // Primero el código exacto (el caso del lector): una consulta por índice.
      const porCodigo = PARECE_CODIGO.test(texto) ? await api.buscarProductoPorCodigo(texto) : null;
      if (porCodigo) return elegir(porCodigo, multiplo);

      // Etiqueta de balanza (EAN-13 que empieza con 2): producto por kg con su peso, sin diálogo.
      if (config?.balanzaActiva && /^2\d{12}$/.test(texto)) {
        const etiqueta = await api.leerCodigoBalanza(texto);
        if (etiqueta) return agregar(etiqueta.producto, etiqueta.cantidadMil);
      }

      const encontrados = await api.buscarProductos(texto, false, 8, 0);
      if (encontrados.length === 1) return elegir(encontrados[0], multiplo);
      if (encontrados.length > 1) {
        setMultiploPendiente(multiplo);
        setResultados(encontrados);
        setAviso(null);
        return;
      }
      setResultados([]);
      setAviso(
        PARECE_CODIGO.test(texto)
          ? { tipo: "no-existe", codigo: texto }
          : { tipo: "error", texto: `No hay productos que coincidan con "${texto}"` },
      );
    } catch (e) {
      setAviso({ tipo: "error", texto: textoError(e) });
    }
  }, [entrada, elegir, agregar, config]);

  const fijarCantidad = (id: number, mil: number) => {
    setUltimoId(id);
    setLineas((ls) =>
      ls.map((l) => (l.producto.id === id ? { ...l, cantidadMil: Math.min(maximoMil(l.producto), mil) } : l)),
    );
  };
  const cambiarUnidades = (id: number, unidades: number) => fijarCantidad(id, Math.max(1, unidades) * 1000);
  const quitar = (id: number) => setLineas((ls) => ls.filter((l) => l.producto.id !== id));

  const alPesar = (mil: number) => {
    if (!pesando) return;
    if (pesando.cambiar) fijarCantidad(pesando.producto.id, mil);
    else agregar(pesando.producto, mil);
    setPesando(null);
  };

  // Total mostrado mientras se escanea. El total que vale lo calcula Rust al confirmar,
  // con los precios de la base de datos.
  const total = lineas.reduce((s, l) => s + subtotalLinea(l.producto.precioVenta, l.cantidadMil), 0);
  // La línea del visor y de la marca: la última tocada o, si ya no está, la última de la lista.
  const ultima = lineas.find((l) => l.producto.id === ultimoId) ?? lineas[lineas.length - 1] ?? null;
  const itemsDelCarrito = (): ItemPedido[] =>
    lineas.map((l) => ({ productoId: l.producto.id, cantidadMil: l.cantidadMil }));

  // --- Ventas en espera ---------------------------------------------------------------------

  const refrescarEspera = useCallback(async () => {
    try {
      const lista = await api.listarEnEspera();
      setEnEspera(lista);
      onEnEspera(lista.length);
    } catch (e) {
      setAviso({ tipo: "error", texto: textoError(e) });
    }
  }, [onEnEspera]);

  // Al abrir la app pueden quedar ventas en espera de la sesión anterior.
  useEffect(() => {
    void refrescarEspera();
  }, [refrescarEspera]);

  const abrirListaEspera = useCallback(() => {
    setDialogo("lista-espera");
    void refrescarEspera(); // totales con los precios del momento
  }, [refrescarEspera]);

  const ponerEnEspera = async (nombre: string | null) => {
    await api.ponerEnEspera(nombre, itemsDelCarrito());
    setLineas([]);
    setDialogo(null);
    setAviso({ tipo: "info", texto: nombre ? `«${nombre}» quedó en espera.` : "La venta quedó en espera.", alerta: false });
    void refrescarEspera();
  };

  const recuperarEspera = async (id: number) => {
    // Si hay algo en la caja, pasa a espera en la misma transacción (intercambio).
    const intercambio = lineas.length > 0 ? { nombre: null, items: itemsDelCarrito() } : null;
    const r = await api.recuperarEnEspera(id, intercambio);
    const porProducto = new Map<number, Linea>();
    for (const l of r.lineas) {
      const previa = porProducto.get(l.producto.id)?.cantidadMil ?? 0;
      porProducto.set(l.producto.id, { producto: l.producto, cantidadMil: previa + l.cantidadMil });
    }
    setLineas([...porProducto.values()]);
    setResultados([]);
    setDialogo(null);
    const partes = ["Venta recuperada."];
    if (intercambio) partes.push("La que estaba en la caja quedó en espera.");
    if (r.saltados > 0) {
      partes.push(
        r.saltados === 1
          ? "Se saltó 1 producto que ya no está activo."
          : `Se saltaron ${r.saltados} productos que ya no están activos.`,
      );
    }
    setAviso({ tipo: "info", texto: partes.join(" "), alerta: intercambio !== null || r.saltados > 0 });
    void refrescarEspera();
  };

  const descartarEspera = async (id: number) => {
    await api.descartarEnEspera(id);
    await refrescarEspera();
  };

  /** Ctrl+P: reimprime la última boleta (una copia, que dice "COPIA"). */
  const reimprimirUltima = useCallback(() => {
    if (ultimaVentaId === null) {
      setAviso({ tipo: "error", texto: "Todavía no hay ventas para reimprimir" });
      return;
    }
    const mostrada = aviso?.tipo === "venta" && aviso.venta.id === ultimaVentaId;
    if (!mostrada || salida?.ventaId !== ultimaVentaId) {
      iniciar(ultimaVentaId, mostrada && aviso?.tipo === "venta" ? aviso.venta.folio : null);
    }
    void imprimir(ultimaVentaId, 1, true);
  }, [ultimaVentaId, aviso, salida, iniciar, imprimir]);

  // Atajos y "foco pegajoso": si el lector escribe mientras el foco está en otro lado,
  // las teclas van igual al campo de escaneo.
  useEffect(() => {
    if (!activa || dialogoAbierto) return;
    const alPresionar = (e: KeyboardEvent) => {
      if (e.key === "F12") {
        e.preventDefault();
        if (lineas.length > 0) setCobrando(true);
        return;
      }
      if (e.ctrlKey && e.key.toLowerCase() === "p") {
        e.preventDefault();
        reimprimirUltima();
        return;
      }
      if (e.key === "F7") {
        e.preventDefault();
        abrirListaEspera();
        return;
      }
      if (e.key === "F8") {
        e.preventDefault();
        if (lineas.length > 0) setDialogo("poner-espera");
        return;
      }
      if (e.key === "Escape") {
        setResultados([]);
        setEntrada("");
        enfocar();
        return;
      }
      const enCampo = e.target instanceof HTMLInputElement;
      if (!enCampo && e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) enfocar();
    };
    window.addEventListener("keydown", alPresionar);
    return () => window.removeEventListener("keydown", alPresionar);
  }, [activa, dialogoAbierto, lineas.length, enfocar, abrirListaEspera, reimprimirUltima]);

  // La venta ya está guardada: la boleta y el cajón van en segundo plano y nada de eso la deshace.
  const alConfirmar = (venta: VentaConfirmada) => {
    setCobrando(false);
    setLineas([]);
    setAviso({ tipo: "venta", venta });
    setUltimaVentaId(venta.id);
    iniciar(venta.id, venta.folio);
    avisarCambioStock();
    if (!config?.impresora) return;
    if (config.cajonActivo && (venta.medioPago === "efectivo" || !config.cajonSoloEfectivo)) {
      void abrirCajon(venta.id);
    }
    if (config.imprimirAlCobrar) void imprimir(venta.id);
  };

  if (cerrada && caja) {
    return (
      <AbrirCaja
        sugerido={caja.ultimoMontoInicial}
        enEspera={enEspera?.length ?? 0}
        onAbierta={() => {
          onCambioCaja();
          setAviso({ tipo: "info", texto: "Caja abierta. ¡Buen turno!", alerta: false });
        }}
      />
    );
  }

  return (
    <div className="flex h-full">
      <section className="flex min-w-0 flex-1 flex-col p-6">
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void procesarEntrada();
          }}
          className="relative"
        >
          <input
            ref={campo}
            value={entrada}
            onChange={(e) => setEntrada(e.target.value)}
            placeholder="Escanea o escribe un producto"
            aria-label="Escanear o escribir un producto"
            className="h-13 w-full rounded-[10px] border-2 border-acento bg-superficie pr-56 pl-4 text-lg shadow-s1 focus-visible:outline-none"
            autoComplete="off"
            spellCheck={false}
          />
          <span className="pointer-events-none absolute inset-y-0 right-4 flex items-center gap-1.5 text-xs text-tenue">
            <Tecla>3*</Tecla> cantidad
            <span className="w-1" />
            <Tecla>0,5*</Tecla> kilos
          </span>
          {resultados.length > 0 && (
            <ul className="absolute z-10 mt-1 w-full overflow-hidden tarjeta shadow-s2">
              {resultados.map((p) => (
                <li key={p.id}>
                  <button
                    type="button"
                    onClick={() => {
                      elegir(p, multiploPendiente);
                      enfocar();
                    }}
                    className="flex h-10 w-full items-center justify-between px-4 text-left hover:bg-suave"
                  >
                    <span>{p.nombre}</span>
                    <span className="text-tenue tabular-nums">
                      {pesos(p.precioVenta)}
                      {p.unidad === "kg" && "/kg"}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </form>

        {aviso?.tipo === "venta" ? (
          <AvisoVenta
            venta={aviso.venta}
            salida={salida?.ventaId === aviso.venta.id ? salida : null}
            reimprimir={() => {
              if (salida?.ventaId !== aviso.venta.id) iniciar(aviso.venta.id, aviso.venta.folio);
              void imprimir(aviso.venta.id, 1, true);
            }}
            reintentar={reintentar}
            guardarPdf={() => {
              if (salida?.ventaId !== aviso.venta.id) iniciar(aviso.venta.id, aviso.venta.folio);
              void guardarPdf(aviso.venta.id, false);
            }}
            abrirPdf={() => void guardarPdf(aviso.venta.id, true)}
            cerrar={cerrar}
          />
        ) : (
          <>
            {aviso && <AvisoCaja aviso={aviso} onCrearProducto={onCrearProducto} />}
            {salida && (
              <AvisoVenta
                venta={null}
                salida={salida}
                reimprimir={() => salida && void imprimir(salida.ventaId, 1, true)}
                reintentar={reintentar}
                guardarPdf={() => salida && void guardarPdf(salida.ventaId, false)}
                abrirPdf={() => salida && void guardarPdf(salida.ventaId, true)}
                cerrar={cerrar}
              />
            )}
          </>
        )}

        <div className="mt-4 min-h-0 flex-1 overflow-auto tarjeta">
          {lineas.length === 0 ? (
            <p className="p-8 text-center text-tenue">Escanea productos para empezar una venta</p>
          ) : (
            <table className="w-full">
              <thead className="sticky top-0 bg-suave text-left text-[13px] text-tenue">
                <tr className="h-10">
                  <th className="px-4 font-medium">Producto</th>
                  <th className="px-4 text-right font-medium">Precio</th>
                  <th className="px-4 text-center font-medium">Cantidad</th>
                  <th className="px-4 text-right font-medium">Subtotal</th>
                  <th className="w-12" />
                </tr>
              </thead>
              <tbody>
                {lineas.map((l) => {
                  const reciente = ultima?.producto.id === l.producto.id;
                  return (
                    <tr key={l.producto.id} className={`h-[46px] border-t border-linea ${reciente ? "bg-resalte" : ""}`}>
                      <td className="relative px-4">
                        {reciente && <span aria-hidden className="absolute inset-y-0 left-0 w-[3px] bg-aviso" />}
                        {l.producto.nombre}
                      </td>
                      <td className="px-4 text-right">
                        {pesos(l.producto.precioVenta)}
                        {l.producto.unidad === "kg" && <span className="text-tenue">/kg</span>}
                      </td>
                      <td className="px-4">
                        {l.producto.unidad === "kg" ? (
                          <div className="mx-auto flex h-8 w-fit items-center gap-2 rounded-lg border border-borde-control bg-superficie pr-1 pl-3">
                            <span>{cantidad(l.cantidadMil, "kg")}</span>
                            <Boton
                              variante="terciario"
                              tamano="chico"
                              onClick={() => setPesando({ producto: l.producto, cambiar: true })}
                              className="h-6 rounded-md bg-seleccion px-2 text-xs font-semibold text-acento hover:text-acento"
                              aria-label={`Cambiar el peso de ${l.producto.nombre}`}
                            >
                              Cambiar
                            </Boton>
                          </div>
                        ) : (
                          <div className="flex items-center justify-center gap-1">
                            <Boton tamano="icono" className="size-7" aria-label={`Una menos de ${l.producto.nombre}`} onClick={() => cambiarUnidades(l.producto.id, l.cantidadMil / 1000 - 1)}>
                              −
                            </Boton>
                            <input
                              value={l.cantidadMil / 1000}
                              onChange={(e) => cambiarUnidades(l.producto.id, Number(e.target.value.replace(/\D/g, "")) || 1)}
                              onKeyDown={(e) => e.key === "Enter" && enfocar()}
                              className="h-7 w-14 rounded-lg border border-borde-control px-1 text-center focus:border-acento"
                              inputMode="numeric"
                              aria-label={`Cantidad de ${l.producto.nombre}`}
                            />
                            <Boton tamano="icono" className="size-7" aria-label={`Una más de ${l.producto.nombre}`} onClick={() => cambiarUnidades(l.producto.id, l.cantidadMil / 1000 + 1)}>
                              +
                            </Boton>
                          </div>
                        )}
                      </td>
                      <td className="px-4 text-right font-medium">
                        {pesos(subtotalLinea(l.producto.precioVenta, l.cantidadMil))}
                      </td>
                      <td className="pr-2 text-center">
                        <Boton tamano="icono" className="size-7" onClick={() => quitar(l.producto.id)} title="Quitar" aria-label={`Quitar ${l.producto.nombre}`}>
                          ×
                        </Boton>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </div>
      </section>

      <aside className="flex w-82 shrink-0 flex-col gap-5 overflow-y-auto border-l border-linea bg-superficie p-5">
        <div className="rounded-panel bg-visor-bg pt-4 pr-[18px] pb-[18px] pl-[18px] text-visor-texto shadow-s2">
          {ultima ? (
            <div>
              <div className="flex items-baseline justify-between gap-3 text-sm font-semibold">
                <span className="min-w-0 truncate">{ultima.producto.nombre}</span>
                <span className="shrink-0 tabular-nums">{pesos(subtotalLinea(ultima.producto.precioVenta, ultima.cantidadMil))}</span>
              </div>
              <div className="mt-0.5 text-[12.5px] text-visor-tenue tabular-nums">
                {ultima.producto.unidad === "kg"
                  ? `${cantidad(ultima.cantidadMil, "kg")} a ${pesos(ultima.producto.precioVenta)} el kilo`
                  : `${cantidad(ultima.cantidadMil, "unidad")} × ${pesos(ultima.producto.precioVenta)}`}
              </div>
            </div>
          ) : (
            <div className="text-[12.5px] text-visor-tenue">Escanea un producto para empezar</div>
          )}
          <hr className="my-3 border-visor-tenue/30" />
          <div className="text-[13px] text-visor-tenue">Total a pagar</div>
          <div role="status" aria-live="polite" className="text-[46px] leading-[1.1] font-bold tabular-nums">
            {pesos(total)}
          </div>
          <div className="mt-1 text-[12.5px] text-visor-tenue">{resumenCarrito(lineas)}</div>
        </div>

        <Boton
          variante="primario"
          tamano="grande"
          bloque
          atajo="F12"
          disabled={lineas.length === 0}
          onClick={() => setCobrando(true)}
        >
          Cobrar
        </Boton>

        <div className="flex flex-col gap-2">
          <h2 className="text-[12.5px] font-semibold text-tenue">Venta actual</h2>
          <Boton
            bloque
            icono={Pause}
            atajo="F8"
            disabled={lineas.length === 0}
            onClick={() => setDialogo("poner-espera")}
            title="Poner en espera (F8)"
            aria-keyshortcuts="F8"
          >
            Poner en espera
          </Boton>
          <Boton bloque icono={Clock} atajo="F7" onClick={abrirListaEspera} aria-keyshortcuts="F7">
            En espera
            <span className="rounded-full bg-seleccion px-2 text-xs font-semibold text-acento tabular-nums">{enEspera?.length ?? 0}</span>
          </Boton>
          <Boton
            variante="peligro"
            bloque
            icono={Trash2}
            disabled={lineas.length === 0}
            onClick={() => {
              setLineas([]);
              enfocar();
            }}
          >
            Cancelar venta
          </Boton>
        </div>

        <div className="flex flex-col gap-2">
          <h2 className="text-[12.5px] font-semibold text-tenue">Caja</h2>
          <Boton bloque icono={ArrowDownUp} disabled={!turno} onClick={() => setDialogo("movimiento")}>
            Ingreso o retiro de efectivo
          </Boton>
          <Boton
            bloque
            icono={Lock}
            disabled={!turno || lineas.length > 0}
            onClick={() => setDialogo("cerrar-caja")}
            title={lineas.length > 0 ? "Cobra, cancela o pon en espera la venta en curso para cerrar la caja" : undefined}
          >
            Cerrar caja
          </Boton>
        </div>

        {/* Con la ventana baja los atajos son lo primero que se esconde. */}
        <ul className="mt-auto flex flex-col gap-1.5 text-xs text-tenue [@media(height<700px)]:hidden">
          <li className="flex items-center gap-2">
            <Tecla>Enter</Tecla> agregar
          </li>
          <li className="flex items-center gap-2">
            <Tecla>Esc</Tecla> limpiar
          </li>
          <li className="flex items-center gap-2">
            <Tecla>Ctrl P</Tecla> reimprimir
          </li>
        </ul>
      </aside>

      {/* Solo mientras la Caja está a la vista: oculta, sus atajos (Enter, Esc) no deben actuar. */}
      {cobrando && activa && (
        <Cobro
          items={lineas.map((l) => ({ productoId: l.producto.id, cantidadMil: l.cantidadMil }))}
          totalEstimado={total}
          redondeoActivo={config?.redondeoEfectivo ?? true}
          onCancelar={() => setCobrando(false)}
          onConfirmada={alConfirmar}
        />
      )}
      {pesando && activa && (
        <DialogoPeso
          producto={pesando.producto}
          inicialMil={
            pesando.cambiar ? lineas.find((l) => l.producto.id === pesando.producto.id)?.cantidadMil : undefined
          }
          onConfirmar={alPesar}
          onCancelar={() => setPesando(null)}
        />
      )}
      {dialogo === "poner-espera" && activa && (
        <PonerEnEspera
          productos={lineas.length}
          total={total}
          onConfirmar={ponerEnEspera}
          onCancelar={() => setDialogo(null)}
        />
      )}
      {dialogo === "movimiento" && activa && turno && (
        <DialogoMovimiento
          turnoId={turno.id}
          onCerrar={() => setDialogo(null)}
          onRegistrado={(m) => {
            setDialogo(null);
            setAviso({
              tipo: "info",
              texto: `${m.tipo === "ingreso" ? "Ingreso" : "Retiro"} de ${pesos(m.monto)} registrado: ${m.motivo}.`,
              alerta: false,
            });
          }}
        />
      )}
      {dialogo === "cerrar-caja" && activa && turno && (
        <CerrarCaja
          turnoId={turno.id}
          imprimir={Boolean(config?.impresora)}
          onImpresion={onImpresion}
          onCancelar={() => setDialogo(null)}
          onCerrada={() => {
            setDialogo(null);
            setAviso(null);
            cerrar();
            onCambioCaja();
          }}
        />
      )}
      {dialogo === "lista-espera" && activa && (
        <ListaEspera
          ventas={enEspera}
          maximo={MAXIMO_EN_ESPERA}
          intercambia={lineas.length > 0}
          onRecuperar={recuperarEspera}
          onDescartar={descartarEspera}
          onCerrar={() => setDialogo(null)}
        />
      )}
    </div>
  );
}

/** "5 productos, 3 por peso" bajo el total del visor. */
function resumenCarrito(lineas: Linea[]): string {
  if (lineas.length === 0) return "Sin productos";
  const porPeso = lineas.filter((l) => l.producto.unidad === "kg").length;
  const productos = `${lineas.length} ${lineas.length === 1 ? "producto" : "productos"}`;
  return porPeso > 0 ? `${productos}, ${porPeso} por peso` : productos;
}

function AvisoCaja({
  aviso,
  onCrearProducto,
}: {
  aviso: Exclude<Aviso, { tipo: "venta" }>;
  onCrearProducto: (codigo: string) => void;
}) {
  if (aviso.tipo === "no-existe") {
    return (
      <div className="mt-3 flex items-center gap-4 rounded-lg bg-aviso-fondo px-4 py-3 text-aviso">
        <span>
          El código <b>{aviso.codigo}</b> no está registrado.
        </span>
        <Boton tamano="chico" onClick={() => onCrearProducto(aviso.codigo)}>
          Crear producto
        </Boton>
      </div>
    );
  }
  if (aviso.tipo === "info") {
    return (
      <div
        className={`mt-3 rounded-lg px-4 py-3 ${aviso.alerta ? "bg-aviso-fondo text-aviso" : "bg-exito-fondo text-exito"}`}
      >
        {aviso.texto}
      </div>
    );
  }
  return <div className="mt-3 rounded-lg bg-peligro-fondo px-4 py-3 text-peligro">{aviso.texto}</div>;
}
