import { useCallback, useEffect, useRef, useState } from "react";
import { api, textoError, type Producto, type VentaConfirmada } from "../api";
import Cobro from "../componentes/Cobro";
import { folio, pesos } from "../formato";

interface Linea {
  producto: Producto;
  cantidad: number;
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
const MAX_CANTIDAD = 100_000;

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
    setLineas((ls) => {
      const i = ls.findIndex((l) => l.producto.id === producto.id);
      if (i === -1) return [...ls, { producto, cantidad }];
      const copia = ls.slice();
      copia[i] = { producto, cantidad: Math.min(MAX_CANTIDAD, copia[i].cantidad + cantidad) };
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

  const cambiarCantidad = (id: number, cantidad: number) =>
    setLineas((ls) =>
      ls.map((l) =>
        l.producto.id === id ? { ...l, cantidad: Math.min(MAX_CANTIDAD, Math.max(1, cantidad)) } : l,
      ),
    );
  const quitar = (id: number) => setLineas((ls) => ls.filter((l) => l.producto.id !== id));

  // Total mostrado mientras se escanea. El total que vale lo calcula Rust al confirmar,
  // con los precios de la base de datos.
  const total = lineas.reduce((s, l) => s + l.producto.precioVenta * l.cantidad, 0);
  const unidades = lineas.reduce((s, l) => s + l.cantidad, 0);

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
            className="w-full rounded-lg border border-slate-300 bg-white px-4 py-3 text-lg outline-none focus:border-slate-900"
            autoComplete="off"
            spellCheck={false}
          />
          {resultados.length > 0 && (
            <ul className="absolute z-10 mt-1 w-full overflow-hidden rounded-lg border border-slate-200 bg-white shadow-lg">
              {resultados.map((p) => (
                <li key={p.id}>
                  <button
                    type="button"
                    onClick={() => {
                      agregar(p, cantidadPendiente);
                      enfocar();
                    }}
                    className="flex w-full justify-between px-4 py-2 text-left hover:bg-slate-100"
                  >
                    <span>{p.nombre}</span>
                    <span className="text-slate-500">{pesos(p.precioVenta)}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </form>

        {aviso && <AvisoCaja aviso={aviso} onCrearProducto={onCrearProducto} />}

        <div className="mt-4 min-h-0 flex-1 overflow-auto rounded-lg border border-slate-200 bg-white">
          {lineas.length === 0 ? (
            <p className="p-8 text-center text-slate-400">Escanea productos para empezar una venta</p>
          ) : (
            <table className="w-full text-sm">
              <thead className="sticky top-0 bg-slate-50 text-left text-xs uppercase text-slate-500">
                <tr>
                  <th className="px-4 py-2">Producto</th>
                  <th className="px-4 py-2 text-right">Precio</th>
                  <th className="px-4 py-2 text-center">Cantidad</th>
                  <th className="px-4 py-2 text-right">Subtotal</th>
                  <th className="w-10" />
                </tr>
              </thead>
              <tbody>
                {lineas.map((l) => (
                  <tr key={l.producto.id} className="border-t border-slate-100">
                    <td className="px-4 py-2">{l.producto.nombre}</td>
                    <td className="px-4 py-2 text-right">{pesos(l.producto.precioVenta)}</td>
                    <td className="px-4 py-2">
                      <div className="flex items-center justify-center gap-1">
                        <BotonChico onClick={() => cambiarCantidad(l.producto.id, l.cantidad - 1)}>−</BotonChico>
                        <input
                          value={l.cantidad}
                          onChange={(e) => cambiarCantidad(l.producto.id, Number(e.target.value.replace(/\D/g, "")) || 1)}
                          onKeyDown={(e) => e.key === "Enter" && enfocar()}
                          className="w-14 rounded border border-slate-200 px-1 py-1 text-center"
                          inputMode="numeric"
                        />
                        <BotonChico onClick={() => cambiarCantidad(l.producto.id, l.cantidad + 1)}>+</BotonChico>
                      </div>
                    </td>
                    <td className="px-4 py-2 text-right font-medium">
                      {pesos(l.producto.precioVenta * l.cantidad)}
                    </td>
                    <td className="pr-2 text-center">
                      <button
                        onClick={() => quitar(l.producto.id)}
                        className="rounded px-2 text-slate-400 hover:bg-red-50 hover:text-red-700"
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

      <aside className="flex w-80 shrink-0 flex-col border-l border-slate-200 bg-white p-6">
        <div className="text-sm text-slate-500">
          {unidades} {unidades === 1 ? "unidad" : "unidades"}
        </div>
        <div className="mt-1 text-sm uppercase tracking-wide text-slate-500">Total</div>
        <div className="text-4xl font-semibold tabular-nums">{pesos(total)}</div>
        <button
          disabled={lineas.length === 0}
          onClick={() => setCobrando(true)}
          className="mt-6 rounded-lg bg-slate-900 py-4 text-lg font-semibold text-white disabled:bg-slate-300"
        >
          Cobrar <span className="text-sm font-normal opacity-70">F12</span>
        </button>
        <button
          disabled={lineas.length === 0}
          onClick={() => {
            setLineas([]);
            enfocar();
          }}
          className="mt-2 rounded-lg py-2 text-sm text-slate-500 hover:bg-slate-100 disabled:opacity-40"
        >
          Cancelar venta
        </button>
        <dl className="mt-auto space-y-1 text-xs text-slate-400">
          <div>Enter: agregar · Esc: limpiar búsqueda</div>
          <div>F1 caja · F2 productos · F12 cobrar</div>
        </dl>
      </aside>

      {cobrando && (
        <Cobro
          items={lineas.map((l) => ({ productoId: l.producto.id, cantidad: l.cantidad }))}
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
      className="h-7 w-7 rounded border border-slate-200 text-slate-600 hover:bg-slate-100"
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
      <div className="mt-3 flex items-baseline gap-4 rounded-lg bg-emerald-50 px-4 py-3 text-emerald-900">
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
      <div className="mt-3 flex items-center gap-4 rounded-lg bg-amber-50 px-4 py-3 text-amber-900">
        <span>
          El código <b>{aviso.codigo}</b> no está registrado.
        </span>
        <button
          onClick={() => onCrearProducto(aviso.codigo)}
          className="rounded bg-amber-900 px-3 py-1 text-sm text-white"
        >
          Crear producto
        </button>
      </div>
    );
  }
  return <div className="mt-3 rounded-lg bg-red-50 px-4 py-3 text-red-800">{aviso.texto}</div>;
}
