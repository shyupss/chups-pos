import { useCallback, useEffect, useRef, useState } from "react";
import { api, textoError, type Producto, type VentaConfirmada } from "../api";
import Cobro from "../componentes/Cobro";
import { cantidad, folio, pesos, subtotalLinea } from "../formato";

interface Linea {
  producto: Producto;
  /** En milésimas: 3 unidades = 3000, 0,532 kg = 532. */
  cantidadMil: number;
}

interface Props {
  activa: boolean;
  onCrearProducto: (codigo: string) => void;
}

type Aviso =
  | { tipo: "error"; texto: string }
  | { tipo: "no-existe"; codigo: string }
  | { tipo: "venta"; venta: VentaConfirmada };

const PARECE_CODIGO = /^[A-Za-z0-9-]+$/;
/** Máximo por línea, en milésimas: 100.000 unidades o 100 kg. */
const maximoMil = (p: Producto) => (p.unidad === "kg" ? 100_000 : 100_000_000);

/** "3*7801234" -> cantidad 3 del código 7801234 */
function separarCantidad(entrada: string): { cantidad: number; texto: string } {
  const m = entrada.match(/^(\d{1,5})\s*[*xX]\s*(.+)$/);
  if (m) return { cantidad: Math.max(1, Number(m[1])), texto: m[2].trim() };
  return { cantidad: 1, texto: entrada.trim() };
}

export default function Caja({ activa, onCrearProducto }: Props) {
  const [lineas, setLineas] = useState<Linea[]>([]);
  const [entrada, setEntrada] = useState("");
  const [resultados, setResultados] = useState<Producto[]>([]);
  const [cantidadPendiente, setCantidadPendiente] = useState(1);
  const [aviso, setAviso] = useState<Aviso | null>(null);
  const [cobrando, setCobrando] = useState(false);
  const campo = useRef<HTMLInputElement>(null);

  const enfocar = useCallback(() => campo.current?.focus(), []);

  useEffect(() => {
    if (activa && !cobrando) enfocar();
  }, [activa, cobrando, enfocar]);

  const agregar = useCallback((producto: Producto, cantidad: number) => {
    const mil = cantidad * 1000;
    setLineas((ls) => {
      const i = ls.findIndex((l) => l.producto.id === producto.id);
      if (i === -1) return [...ls, { producto, cantidadMil: mil }];
      const copia = ls.slice();
      copia[i] = { producto, cantidadMil: Math.min(maximoMil(producto), copia[i].cantidadMil + mil) };
      return copia;
    });
    setResultados([]);
    setAviso(null);
  }, []);

  const procesarEntrada = useCallback(async () => {
    const { cantidad, texto } = separarCantidad(entrada);
    setEntrada("");
    if (!texto) return;
    try {
      // Primero el código exacto (el caso del lector): una consulta por índice.
      const porCodigo = PARECE_CODIGO.test(texto) ? await api.buscarProductoPorCodigo(texto) : null;
      if (porCodigo) return agregar(porCodigo, cantidad);

      const encontrados = await api.buscarProductos(texto, false, 8, 0);
      if (encontrados.length === 1) return agregar(encontrados[0], cantidad);
      if (encontrados.length > 1) {
        setCantidadPendiente(cantidad);
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
  }, [entrada, agregar]);

  const cambiarUnidades = (id: number, unidades: number) =>
    setLineas((ls) =>
      ls.map((l) =>
        l.producto.id === id
          ? { ...l, cantidadMil: Math.min(maximoMil(l.producto), Math.max(1, unidades) * 1000) }
          : l,
      ),
    );
  const quitar = (id: number) => setLineas((ls) => ls.filter((l) => l.producto.id !== id));

  // Total mostrado mientras se escanea. El total que vale lo calcula Rust al confirmar,
  // con los precios de la base de datos.
  const total = lineas.reduce((s, l) => s + subtotalLinea(l.producto.precioVenta, l.cantidadMil), 0);

  // Atajos y "foco pegajoso": si el lector escribe mientras el foco está en otro lado,
  // las teclas van igual al campo de escaneo.
  useEffect(() => {
    if (!activa || cobrando) return;
    const alPresionar = (e: KeyboardEvent) => {
      if (e.key === "F12") {
        e.preventDefault();
        if (lineas.length > 0) setCobrando(true);
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
  }, [activa, cobrando, lineas.length, enfocar]);

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
            placeholder="Escanea un código o escribe un nombre y presiona Enter  (3*código = 3 unidades)"
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
                      agregar(p, cantidadPendiente);
                      enfocar();
                    }}
                    className="flex h-10 w-full items-center justify-between px-4 text-left hover:bg-fondo"
                  >
                    <span>{p.nombre}</span>
                    <span className="text-tenue tabular-nums">{pesos(p.precioVenta)}</span>
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
                        <div className="text-center">{cantidad(l.cantidadMil, "kg")}</div>
                      ) : (
                        <div className="flex items-center justify-center gap-1">
                          <BotonChico onClick={() => cambiarUnidades(l.producto.id, l.cantidadMil / 1000 - 1)}>−</BotonChico>
                          <input
                            value={l.cantidadMil / 1000}
                            onChange={(e) => cambiarUnidades(l.producto.id, Number(e.target.value.replace(/\D/g, "")) || 1)}
                            onKeyDown={(e) => e.key === "Enter" && enfocar()}
                            className="h-7 w-14 rounded-lg border border-linea bg-superficie px-1 text-center outline-none focus:border-texto"
                            inputMode="numeric"
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
          {/* Reserva su lugar en el diseño; la venta en espera llega en la fase 2. */}
          <button disabled title="Llega en la fase 2" className="h-10 flex-1 rounded-lg text-tenue opacity-50">
            Poner en espera
          </button>
        </div>
        <dl className="mt-auto space-y-1 text-xs text-tenue">
          <div>Enter: agregar · Esc: limpiar búsqueda</div>
          <div>F1 caja · F2 productos · F12 cobrar</div>
        </dl>
      </aside>

      {/* Solo mientras la Caja está a la vista: oculta, sus atajos (Enter, Esc) no deben actuar. */}
      {cobrando && activa && (
        <Cobro
          items={lineas.map((l) => ({ productoId: l.producto.id, cantidadMil: l.cantidadMil }))}
          totalEstimado={total}
          onCancelar={() => setCobrando(false)}
          onConfirmada={alConfirmar}
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
  return <div className="mt-3 rounded-lg bg-peligro/10 px-4 py-3 text-peligro">{aviso.texto}</div>;
}
