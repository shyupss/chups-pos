import { useCallback, useEffect, useRef, useState } from "react";
import { api, textoError, type ItemPedido, type Producto, type ResumenEspera, type VentaConfirmada } from "../api";
import Cobro from "../componentes/Cobro";
import DialogoPeso from "../componentes/DialogoPeso";
import ListaEspera from "../componentes/ListaEspera";
import PonerEnEspera from "../componentes/PonerEnEspera";
import { cantidad, folio, MAX_KG_MIL, MAX_UNIDADES_MIL, parseKg, pesos, subtotalLinea } from "../formato";

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

export default function Caja({ activa, onCrearProducto, onEnEspera }: Props) {
  const [lineas, setLineas] = useState<Linea[]>([]);
  const [entrada, setEntrada] = useState("");
  const [resultados, setResultados] = useState<Producto[]>([]);
  const [multiploPendiente, setMultiploPendiente] = useState<Multiplo | null>(null);
  const [aviso, setAviso] = useState<Aviso | null>(null);
  const [cobrando, setCobrando] = useState(false);
  const [pesando, setPesando] = useState<Pesando | null>(null);
  const [redondeoActivo, setRedondeoActivo] = useState(true);
  const [dialogo, setDialogo] = useState<"poner-espera" | "lista-espera" | null>(null);
  const [enEspera, setEnEspera] = useState<ResumenEspera[] | null>(null);
  const campo = useRef<HTMLInputElement>(null);

  const dialogoAbierto = cobrando || pesando !== null || dialogo !== null;
  const enfocar = useCallback(() => campo.current?.focus(), []);

  useEffect(() => {
    if (activa && !dialogoAbierto) enfocar();
  }, [activa, dialogoAbierto, enfocar]);

  // Se vuelve a leer al entrar a la Caja, por si cambió en Configuración.
  useEffect(() => {
    if (!activa) return;
    api
      .obtenerConfig()
      .then((c) => setRedondeoActivo(c.redondeoEfectivo))
      .catch(() => {});
  }, [activa]);

  const agregar = useCallback((producto: Producto, mil: number) => {
    setLineas((ls) => {
      const i = ls.findIndex((l) => l.producto.id === producto.id);
      if (i === -1) return [...ls, { producto, cantidadMil: Math.min(maximoMil(producto), mil) }];
      const copia = ls.slice();
      copia[i] = { producto, cantidadMil: Math.min(maximoMil(producto), copia[i].cantidadMil + mil) };
      return copia;
    });
    setResultados([]);
    setAviso(null);
  }, []);

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
  }, [entrada, elegir]);

  const fijarCantidad = (id: number, mil: number) =>
    setLineas((ls) =>
      ls.map((l) => (l.producto.id === id ? { ...l, cantidadMil: Math.min(maximoMil(l.producto), mil) } : l)),
    );
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
  }, [activa, dialogoAbierto, lineas.length, enfocar, abrirListaEspera]);

  const alConfirmar = (venta: VentaConfirmada) => {
    setCobrando(false);
    setLineas([]);
    setAviso({ tipo: "venta", venta });
  };

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
            placeholder="Escanea o escribe y presiona Enter  (3*código = 3 unidades, 0,5*código = 0,5 kg)"
            className="h-12 w-full rounded-lg border-2 border-acento bg-superficie px-4 text-lg outline-none"
            autoComplete="off"
            spellCheck={false}
          />
          {resultados.length > 0 && (
            <ul className="absolute z-10 mt-1 w-full overflow-hidden rounded-lg border border-linea bg-superficie">
              {resultados.map((p) => (
                <li key={p.id}>
                  <button
                    type="button"
                    onClick={() => {
                      elegir(p, multiploPendiente);
                      enfocar();
                    }}
                    className="flex h-10 w-full items-center justify-between px-4 text-left hover:bg-fondo"
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

        {aviso && <AvisoCaja aviso={aviso} onCrearProducto={onCrearProducto} />}

        <div className="mt-4 min-h-0 flex-1 overflow-auto rounded-lg border border-linea bg-superficie">
          {lineas.length === 0 ? (
            <p className="p-8 text-center text-tenue">Escanea productos para empezar una venta</p>
          ) : (
            <table className="w-full">
              <thead className="sticky top-0 bg-superficie text-left text-xs uppercase text-tenue">
                <tr className="h-10">
                  <th className="px-4 font-medium">Producto</th>
                  <th className="px-4 text-right font-medium">Precio</th>
                  <th className="px-4 text-center font-medium">Cantidad</th>
                  <th className="px-4 text-right font-medium">Subtotal</th>
                  <th className="w-10" />
                </tr>
              </thead>
              <tbody>
                {lineas.map((l) => (
                  <tr key={l.producto.id} className="h-10 border-t border-linea">
                    <td className="px-4">{l.producto.nombre}</td>
                    <td className="px-4 text-right">
                      {pesos(l.producto.precioVenta)}
                      {l.producto.unidad === "kg" && <span className="text-tenue">/kg</span>}
                    </td>
                    <td className="px-4">
                      {l.producto.unidad === "kg" ? (
                        <div className="flex items-center justify-center gap-2">
                          <span>{cantidad(l.cantidadMil, "kg")}</span>
                          <button
                            type="button"
                            onClick={() => setPesando({ producto: l.producto, cambiar: true })}
                            className="h-7 rounded-lg border border-linea px-2 text-xs text-tenue hover:bg-fondo hover:text-texto"
                          >
                            Cambiar peso
                          </button>
                        </div>
                      ) : (
                        <div className="flex items-center justify-center gap-1">
                          <BotonChico onClick={() => cambiarUnidades(l.producto.id, l.cantidadMil / 1000 - 1)}>−</BotonChico>
                          <input
                            value={l.cantidadMil / 1000}
                            onChange={(e) => cambiarUnidades(l.producto.id, Number(e.target.value.replace(/\D/g, "")) || 1)}
                            onKeyDown={(e) => e.key === "Enter" && enfocar()}
                            className="h-7 w-14 rounded-lg border border-linea bg-superficie px-1 text-center outline-none focus:border-texto"
                            inputMode="numeric"
                            aria-label={`Cantidad de ${l.producto.nombre}`}
                          />
                          <BotonChico onClick={() => cambiarUnidades(l.producto.id, l.cantidadMil / 1000 + 1)}>+</BotonChico>
                        </div>
                      )}
                    </td>
                    <td className="px-4 text-right font-medium">
                      {pesos(subtotalLinea(l.producto.precioVenta, l.cantidadMil))}
                    </td>
                    <td className="pr-2 text-center">
                      <button
                        onClick={() => quitar(l.producto.id)}
                        className="h-7 rounded-lg px-2 text-tenue hover:bg-peligro/10 hover:text-peligro"
                        title="Quitar"
                      >
                        ×
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </section>

      <aside className="flex w-70 shrink-0 flex-col border-l border-linea bg-superficie p-5">
        <div className="text-tenue">
          {lineas.length} {lineas.length === 1 ? "producto" : "productos"}
        </div>
        <div className="mt-1 text-xs font-medium uppercase tracking-wide text-tenue">Total</div>
        <div className="text-4xl font-semibold tabular-nums">{pesos(total)}</div>
        <button
          disabled={lineas.length === 0}
          onClick={() => setCobrando(true)}
          className="mt-6 h-13 rounded-lg bg-acento text-lg font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
        >
          Cobrar <span className="text-sm font-normal opacity-70">F12</span>
        </button>
        <div className="mt-2 flex gap-2">
          <button
            disabled={lineas.length === 0}
            onClick={() => {
              setLineas([]);
              enfocar();
            }}
            className="h-10 flex-1 rounded-lg text-tenue enabled:hover:bg-fondo enabled:hover:text-texto disabled:opacity-50"
          >
            Cancelar venta
          </button>
          <button
            disabled={lineas.length === 0}
            onClick={() => setDialogo("poner-espera")}
            title="Poner en espera (F8)"
            aria-keyshortcuts="F8"
            className="h-10 flex-1 rounded-lg text-tenue enabled:hover:bg-fondo enabled:hover:text-texto disabled:opacity-50"
          >
            Poner en espera
          </button>
        </div>
        <button
          onClick={abrirListaEspera}
          aria-keyshortcuts="F7"
          className="mt-2 flex h-10 items-center justify-between rounded-lg border border-linea px-3 hover:bg-fondo"
        >
          <span>
            En espera <span className="tabular-nums">({enEspera?.length ?? 0})</span>
          </span>
          <span className="text-xs text-tenue">F7</span>
        </button>
        <dl className="mt-auto space-y-1 text-xs text-tenue">
          <div>Enter: agregar · Esc: limpiar búsqueda</div>
          <div>F7 en espera · F8 poner en espera</div>
          <div>F1 caja · F2 productos · F12 cobrar</div>
        </dl>
      </aside>

      {/* Solo mientras la Caja está a la vista: oculta, sus atajos (Enter, Esc) no deben actuar. */}
      {cobrando && activa && (
        <Cobro
          items={lineas.map((l) => ({ productoId: l.producto.id, cantidadMil: l.cantidadMil }))}
          totalEstimado={total}
          redondeoActivo={redondeoActivo}
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

function BotonChico({ onClick, children }: { onClick: () => void; children: string }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="size-7 rounded-lg border border-linea text-tenue hover:bg-fondo hover:text-texto"
    >
      {children}
    </button>
  );
}

function AvisoCaja({
  aviso,
  onCrearProducto,
}: {
  aviso: Aviso;
  onCrearProducto: (codigo: string) => void;
}) {
  if (aviso.tipo === "venta") {
    const v = aviso.venta;
    return (
      <div className="mt-3 flex items-baseline gap-4 rounded-lg bg-exito-fondo px-4 py-3 text-exito tabular-nums">
        <span className="font-semibold">Venta {folio(v.folio)} registrada</span>
        <span>Total {pesos(v.total)}</span>
        {v.medioPago === "efectivo" && v.vuelto !== null && (
          <span className="text-lg font-semibold">Vuelto {pesos(v.vuelto)}</span>
        )}
      </div>
    );
  }
  if (aviso.tipo === "no-existe") {
    return (
      <div className="mt-3 flex items-center gap-4 rounded-lg bg-aviso-fondo px-4 py-3 text-aviso">
        <span>
          El código <b>{aviso.codigo}</b> no está registrado.
        </span>
        <button
          onClick={() => onCrearProducto(aviso.codigo)}
          className="h-8 rounded-lg bg-aviso px-3 font-medium text-aviso-fondo"
        >
          Crear producto
        </button>
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
  return <div className="mt-3 rounded-lg bg-peligro/10 px-4 py-3 text-peligro">{aviso.texto}</div>;
}
