import { useEffect, useRef, useState } from "react";
import { api, textoError, type Categoria, type DetalleCompra, type Producto, type Proveedor } from "../api";
import { aEntero, parseCantidad, pesos, subtotalLinea } from "../formato";
import Dialogo from "./Dialogo";
import FormularioProducto from "./FormularioProducto";
import { esCancelado, useConPin } from "./PedirPin";

interface Linea {
  producto: Producto;
  /** Lo escrito: unidades enteras o kg ("2,5"). */
  cantidad: string;
  costo: string;
}

interface Props {
  onRegistrada: (d: DetalleCompra) => void;
  onCerrar: () => void;
}

const PARECE_CODIGO = /^[A-Za-z0-9-]+$/;
const claseCampo = "h-10 w-full rounded-lg border border-linea bg-fondo px-3 outline-none focus:border-texto";

/**
 * Registrar una compra: proveedor y documento, y las líneas escaneando o buscando, cada una con
 * cantidad y costo unitario. El total en vivo es una estimación; el que vale lo calcula Rust.
 */
export default function RegistrarCompra({ onRegistrada, onCerrar }: Props) {
  const conPin = useConPin();
  const [proveedores, setProveedores] = useState<Proveedor[]>([]);
  const [proveedorId, setProveedorId] = useState<string>("");
  const [documento, setDocumento] = useState("");
  const [nota, setNota] = useState("");
  const [lineas, setLineas] = useState<Linea[]>([]);
  const [entrada, setEntrada] = useState("");
  const [resultados, setResultados] = useState<Producto[]>([]);
  const [noExiste, setNoExiste] = useState<string | null>(null);
  const [creando, setCreando] = useState<string | null>(null);
  const [categorias, setCategorias] = useState<Categoria[]>([]);
  const [conCaja, setConCaja] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);
  const escaner = useRef<HTMLInputElement>(null);
  const cantidades = useRef(new Map<number, HTMLInputElement>());

  useEffect(() => {
    api.listarProveedores("", false, 200, 0).then(setProveedores).catch(() => {});
    api.listarCategorias().then(setCategorias).catch(() => {});
  }, []);

  const agregar = (p: Producto) => {
    setResultados([]);
    setNoExiste(null);
    setError(null);
    setLineas((ls) =>
      ls.some((l) => l.producto.id === p.id)
        ? ls
        : [...ls, { producto: p, cantidad: "", costo: p.costo !== null ? String(p.costo) : "" }],
    );
    // El foco va a la cantidad de esa línea.
    setTimeout(() => cantidades.current.get(p.id)?.focus(), 0);
  };

  const buscar = async () => {
    const texto = entrada.trim();
    setEntrada("");
    if (!texto) return;
    try {
      const porCodigo = PARECE_CODIGO.test(texto) ? await api.buscarProductoPorCodigo(texto) : null;
      if (porCodigo) return agregar(porCodigo);
      const encontrados = await api.buscarProductos(texto, false, 8, 0);
      if (encontrados.length === 1) return agregar(encontrados[0]);
      if (encontrados.length > 1) return setResultados(encontrados);
      if (PARECE_CODIGO.test(texto)) setNoExiste(texto);
      else setError(`No hay productos que coincidan con "${texto}"`);
    } catch (e) {
      setError(textoError(e));
    }
  };

  const cambiar = (id: number, cambio: Partial<Linea>) =>
    setLineas((ls) => ls.map((l) => (l.producto.id === id ? { ...l, ...cambio } : l)));

  const leidas = lineas.map((l) => ({
    l,
    mil: parseCantidad(l.cantidad, l.producto.unidad),
    costo: aEntero(l.costo),
  }));
  const completas = leidas.every((x) => x.mil !== null && x.mil > 0 && x.costo !== null);
  const total = leidas.reduce((s, x) => s + (x.mil && x.costo !== null ? subtotalLinea(x.costo, x.mil) : 0), 0);
  const proveedor = proveedores.find((p) => String(p.id) === proveedorId);

  const registrar = async () => {
    if (enviando || lineas.length === 0 || !completas) return;
    setEnviando(true);
    setError(null);
    try {
      const d = await conPin(
        (pin) =>
          api.registrarCompra(
            {
              proveedorId: proveedor ? proveedor.id : null,
              documento: documento.trim() || null,
              nota: nota.trim() || null,
              lineas: leidas.map((x) => ({ productoId: x.l.producto.id, cantidadMil: x.mil!, costoUnitario: x.costo! })),
              pagadaConCaja: conCaja,
            },
            pin,
          ),
        { accion: `Pagar ${pesos(total)} de la compra con la caja` },
      );
      onRegistrada(d);
    } catch (e) {
      if (!esCancelado(e)) setError(textoError(e));
      setEnviando(false);
    }
  };

  return (
    <Dialogo titulo="Registrar compra" onCerrar={onCerrar} ancho="w-[58rem]">
      <div className="mt-4 grid grid-cols-[1fr_14rem] gap-3">
        <label className="block">
          <span className="text-xs text-tenue">Proveedor</span>
          <select value={proveedorId} onChange={(e) => setProveedorId(e.target.value)} className={`mt-1 ${claseCampo}`}>
            <option value="">Sin proveedor</option>
            {proveedores.map((p) => (
              <option key={p.id} value={p.id}>
                {p.nombre}
              </option>
            ))}
          </select>
        </label>
        <label className="block">
          <span className="text-xs text-tenue">N° de factura o guía (opcional)</span>
          <input value={documento} onChange={(e) => setDocumento(e.target.value)} maxLength={40} className={`mt-1 ${claseCampo}`} />
        </label>
      </div>

      <form
        onSubmit={(e) => {
          e.preventDefault();
          void buscar();
        }}
        className="relative mt-4"
      >
        <input
          ref={escaner}
          autoFocus
          value={entrada}
          onChange={(e) => setEntrada(e.target.value)}
          placeholder="Escanea o busca un producto y Enter"
          aria-label="Agregar producto a la compra"
          autoComplete="off"
          className="h-11 w-full rounded-lg border-2 border-acento bg-superficie px-4 outline-none"
        />
        {resultados.length > 0 && (
          <ul className="absolute z-10 mt-1 w-full overflow-hidden rounded-lg border border-linea bg-superficie">
            {resultados.map((p) => (
              <li key={p.id}>
                <button type="button" onClick={() => agregar(p)} className="h-10 w-full px-4 text-left hover:bg-fondo">
                  {p.nombre}
                </button>
              </li>
            ))}
          </ul>
        )}
      </form>
      {noExiste && (
        <div className="mt-2 flex items-center gap-3 rounded-lg bg-aviso-fondo px-3 py-2 text-aviso">
          <span>
            El código <b>{noExiste}</b> no está registrado.
          </span>
          <button
            type="button"
            onClick={() => {
              setCreando(noExiste);
              setNoExiste(null);
            }}
            className="h-8 rounded-lg bg-aviso px-3 font-medium text-aviso-fondo"
          >
            Crear producto
          </button>
        </div>
      )}

      <div className="mt-3 max-h-80 min-h-28 overflow-auto rounded-lg border border-linea">
        {lineas.length === 0 ? (
          <p className="p-8 text-center text-tenue">Escanea los productos que llegaron.</p>
        ) : (
          <table className="w-full">
            <thead className="sticky top-0 bg-superficie text-left text-xs uppercase text-tenue">
              <tr className="h-9">
                <th className="px-3 font-medium">Producto</th>
                <th className="w-32 px-3 font-medium">Cantidad</th>
                <th className="w-36 px-3 font-medium">Costo unitario</th>
                <th className="w-28 px-3 text-right font-medium">Subtotal</th>
                <th className="w-10" />
              </tr>
            </thead>
            <tbody>
              {leidas.map(({ l, mil, costo }) => {
                const kg = l.producto.unidad === "kg";
                return (
                  <tr key={l.producto.id} className="h-12 border-t border-linea">
                    <td className="max-w-64 truncate px-3">{l.producto.nombre}</td>
                    <td className="px-3">
                      <input
                        ref={(el) => {
                          if (el) cantidades.current.set(l.producto.id, el);
                          else cantidades.current.delete(l.producto.id);
                        }}
                        value={l.cantidad}
                        onChange={(e) => cambiar(l.producto.id, { cantidad: e.target.value })}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            e.preventDefault();
                            escaner.current?.focus();
                          }
                        }}
                        inputMode="decimal"
                        placeholder={kg ? "kg" : "unidades"}
                        aria-label={`Cantidad de ${l.producto.nombre}`}
                        aria-invalid={l.cantidad !== "" && (mil === null || mil <= 0)}
                        className="h-9 w-full rounded-lg border border-linea bg-fondo px-2 text-right tabular-nums outline-none focus:border-texto aria-invalid:border-peligro"
                      />
                    </td>
                    <td className="px-3">
                      <div className="flex items-center gap-1">
                        <input
                          value={l.costo ? pesos(costo ?? 0) : ""}
                          onChange={(e) => cambiar(l.producto.id, { costo: e.target.value })}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") {
                              e.preventDefault();
                              escaner.current?.focus();
                            }
                          }}
                          inputMode="numeric"
                          placeholder="$0"
                          aria-label={`Costo de ${l.producto.nombre}`}
                          className="h-9 w-full rounded-lg border border-linea bg-fondo px-2 text-right tabular-nums outline-none focus:border-texto"
                        />
                        <span className="w-6 text-xs text-tenue">{kg ? "/kg" : "c/u"}</span>
                      </div>
                    </td>
                    <td className="px-3 text-right tabular-nums">
                      {mil && costo !== null ? pesos(subtotalLinea(costo, mil)) : "—"}
                    </td>
                    <td className="pr-2 text-center">
                      <button
                        type="button"
                        onClick={() => setLineas((ls) => ls.filter((x) => x.producto.id !== l.producto.id))}
                        aria-label={`Quitar ${l.producto.nombre}`}
                        className="h-7 rounded-lg px-2 text-tenue hover:bg-peligro/10 hover:text-peligro"
                      >
                        ×
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      <div className="mt-4 grid grid-cols-[1fr_auto] items-end gap-6">
        <div className="space-y-3">
          <label className="flex items-center gap-2">
            <input type="checkbox" checked={conCaja} onChange={(e) => setConCaja(e.target.checked)} className="size-4" />
            Pagada con efectivo de la caja
            <span className="text-xs text-tenue">(un retiro en el turno abierto)</span>
          </label>
          <input
            value={nota}
            onChange={(e) => setNota(e.target.value)}
            maxLength={200}
            placeholder="Nota (opcional)"
            aria-label="Nota"
            className={claseCampo}
          />
        </div>
        <div className="text-right tabular-nums">
          <div className="text-xs font-medium uppercase text-tenue">Total</div>
          <div className="text-3xl font-semibold">{pesos(total)}</div>
        </div>
      </div>
      {error && <p className="mt-3 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}
      <div className="mt-5 flex gap-2">
        <button type="button" onClick={onCerrar} className="h-11 flex-1 rounded-lg text-tenue hover:bg-fondo hover:text-texto">
          Cancelar <span className="text-xs opacity-60">Esc</span>
        </button>
        <button
          type="button"
          onClick={() => void registrar()}
          disabled={enviando || lineas.length === 0 || !completas}
          className="h-11 flex-[2] rounded-lg bg-acento font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
        >
          Registrar compra {lineas.length > 0 && `· ${lineas.length} ${lineas.length === 1 ? "producto" : "productos"}`}
        </button>
      </div>

      {creando !== null && (
        <div
          className="fixed inset-0 z-30 flex items-center justify-center bg-velo p-4"
          onKeyDown={(e) => e.key === "Escape" && e.stopPropagation()}
        >
          <div role="dialog" aria-modal="true" className="h-[42rem] max-h-full w-[28rem] rounded-lg border border-linea bg-superficie">
            <FormularioProducto
              producto={null}
              codigoInicial={creando}
              categorias={categorias}
              categoriaInicial={null}
              onGuardado={(p) => {
                setCreando(null);
                agregar(p);
              }}
              onCerrar={() => {
                setCreando(null);
                escaner.current?.focus();
              }}
            />
          </div>
        </div>
      )}
    </Dialogo>
  );
}
