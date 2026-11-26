import Boton from "./Boton";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { useAtajos } from "../atajos";
import { api, textoError, type Categoria, type DetalleCompra, type Producto, type Proveedor } from "../api";
import { aEntero, parseCantidad, pesos, subtotalLinea } from "../formato";
import CampoProveedor from "./CampoProveedor";
import { mensajeSinProducto } from "./compra-nucleo";
import Dialogo from "./Dialogo";
import FormularioProducto from "./FormularioProducto";
import { esCancelado, useConPin } from "./PedirPin";
import Tabla, { type ColumnaTabla } from "./Tabla";

interface Linea {
  producto: Producto;
  /** Lo escrito: unidades enteras o kg ("2,5"). */
  cantidad: string;
  costo: string;
}

/** Una línea con lo escrito ya leído: la cantidad en milésimas y el costo en pesos. */
interface LineaLeida {
  l: Linea;
  mil: number | null;
  costo: number | null;
}

interface Props {
  onRegistrada: (d: DetalleCompra) => void;
  onCerrar: () => void;
}

const PARECE_CODIGO = /^[A-Za-z0-9-]+$/;
const claseCampo = "h-11 w-full rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento";

/**
 * Registrar una compra, en tres pasos de arriba hacia abajo: 1) a quién le compraste, 2) los productos que
 * llegaron (escaneando o buscando) y 3) cómo se pagó, con el total. Ocupa casi toda la pantalla para que la
 * tabla de productos tenga lugar. El total en vivo es una estimación; el que vale lo calcula Rust.
 */
export default function RegistrarCompra({ onRegistrada, onCerrar }: Props) {
  const conPin = useConPin();
  const [proveedor, setProveedor] = useState<Proveedor | null>(null);
  const [documento, setDocumento] = useState("");
  const [nota, setNota] = useState("");
  const [lineas, setLineas] = useState<Linea[]>([]);
  const [entrada, setEntrada] = useState("");
  const [resultados, setResultados] = useState<Producto[]>([]);
  const [sinProducto, setSinProducto] = useState<ReturnType<typeof mensajeSinProducto> | null>(null);
  const [creando, setCreando] = useState<{ codigo?: string; nombre?: string } | null>(null);
  const [categorias, setCategorias] = useState<Categoria[]>([]);
  // null mientras se pregunta si hay un turno abierto: pagar con la caja solo se puede con uno.
  const [hayTurno, setHayTurno] = useState<boolean | null>(null);
  const [conCaja, setConCaja] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);
  const [irAlFinal, setIrAlFinal] = useState(0);
  const escaner = useRef<HTMLInputElement>(null);
  const cantidades = useRef(new Map<number, HTMLInputElement>());
  // La línea cuya cantidad se enfoca apenas se dibuje (la tabla solo dibuja las filas que se ven).
  const pendienteFoco = useRef<number | null>(null);

  useEffect(() => {
    api.listarCategorias().then(setCategorias).catch(() => {});
    api
      .estadoCaja()
      .then((c) => setHayTurno(c.turno !== null))
      .catch(() => setHayTurno(false));
  }, []);

  const agregar = (p: Producto) => {
    setResultados([]);
    setSinProducto(null);
    setError(null);
    const yaEsta = lineas.some((l) => l.producto.id === p.id);
    if (!yaEsta) {
      setLineas((ls) => [...ls, { producto: p, cantidad: "", costo: p.costo !== null ? String(p.costo) : "" }]);
      setIrAlFinal((n) => n + 1);
    }
    // El foco va a la cantidad de esa línea: ya en pantalla, o apenas se dibuje.
    const campo = cantidades.current.get(p.id);
    if (campo) campo.focus();
    else pendienteFoco.current = p.id;
  };

  const buscar = async () => {
    const texto = entrada.trim();
    setEntrada("");
    setSinProducto(null);
    if (!texto) return;
    try {
      const porCodigo = PARECE_CODIGO.test(texto) ? await api.buscarProductoPorCodigo(texto) : null;
      if (porCodigo) return agregar(porCodigo);
      const encontrados = await api.buscarProductos(texto, false, 8, 0);
      if (encontrados.length === 1) return agregar(encontrados[0]);
      if (encontrados.length > 1) return setResultados(encontrados);
      // Ni por código ni por nombre: se ofrece crearlo con lo escrito (el código o el nombre).
      setSinProducto(mensajeSinProducto(texto));
    } catch (e) {
      setError(textoError(e));
    }
  };

  const cambiar = (id: number, cambio: Partial<Linea>) =>
    setLineas((ls) => ls.map((l) => (l.producto.id === id ? { ...l, ...cambio } : l)));

  const leidas: LineaLeida[] = lineas.map((l) => ({
    l,
    mil: parseCantidad(l.cantidad, l.producto.unidad),
    costo: aEntero(l.costo),
  }));
  const completas = leidas.every((x) => x.mil !== null && x.mil > 0 && x.costo !== null);
  const total = leidas.reduce((s, x) => s + (x.mil && x.costo !== null ? subtotalLinea(x.costo, x.mil) : 0), 0);

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

  const enter = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      escaner.current?.focus();
    }
  };

  // Producto · Cantidad · Costo · Total · Quitar: lo que se escribe va al medio, con campos de 44 px.
  const columnas: ColumnaTabla<LineaLeida>[] = [
    { id: "producto", titulo: "Producto", ancho: "minmax(10rem,1fr)", celda: ({ l }) => <span title={l.producto.nombre}>{l.producto.nombre}</span> },
    {
      id: "cantidad",
      titulo: "Cantidad",
      ancho: "9rem",
      alinear: "derecha",
      sinPuntos: true,
      celda: ({ l, mil }) => (
        <input
          ref={(el) => {
            if (!el) return void cantidades.current.delete(l.producto.id);
            cantidades.current.set(l.producto.id, el);
            if (pendienteFoco.current === l.producto.id) {
              pendienteFoco.current = null;
              el.focus();
            }
          }}
          value={l.cantidad}
          onChange={(e) => cambiar(l.producto.id, { cantidad: e.target.value })}
          onKeyDown={enter}
          inputMode="decimal"
          placeholder={l.producto.unidad === "kg" ? "kg" : "unidades"}
          aria-label={`Cantidad de ${l.producto.nombre}`}
          aria-invalid={l.cantidad !== "" && (mil === null || mil <= 0)}
          className="h-11 w-full rounded-lg border border-borde-control bg-superficie px-2 text-right tabular-nums focus:border-acento aria-invalid:border-peligro"
        />
      ),
    },
    {
      id: "costo",
      titulo: "Costo unitario",
      ancho: "11rem",
      alinear: "derecha",
      sinPuntos: true,
      celda: ({ l, costo }) => (
        <span className="flex items-center gap-2">
          <input
            value={l.costo ? pesos(costo ?? 0) : ""}
            onChange={(e) => cambiar(l.producto.id, { costo: e.target.value })}
            onKeyDown={enter}
            inputMode="numeric"
            placeholder="$0"
            aria-label={`Costo de ${l.producto.nombre}`}
            className="h-11 w-full min-w-0 rounded-lg border border-borde-control bg-superficie px-2 text-right tabular-nums focus:border-acento"
          />
          <span className="w-8 shrink-0 text-left text-sm text-tenue">{l.producto.unidad === "kg" ? "/kg" : "c/u"}</span>
        </span>
      ),
    },
    {
      id: "total",
      titulo: "Total",
      ancho: "7.5rem",
      alinear: "derecha",
      celda: ({ mil, costo }) => (mil && costo !== null ? pesos(subtotalLinea(costo, mil)) : "—"),
      clase: () => "font-medium",
    },
    {
      id: "quitar",
      titulo: "",
      ancho: "6rem",
      alinear: "derecha",
      sinPuntos: true,
      celda: ({ l }) => (
        <Boton
          variante="terciario"
          tamano="chico"
          onClick={() => setLineas((ls) => ls.filter((x) => x.producto.id !== l.producto.id))}
          aria-label={`Quitar ${l.producto.nombre}`}
        >
          Quitar
        </Boton>
      ),
    },
  ];

  return (
    <Dialogo titulo="Registrar compra" onCerrar={onCerrar} ancho="w-[92vw]" alto="h-[92vh]">
      <div className="mt-2 flex min-h-0 flex-1 flex-col gap-3">
       {/* Si el alto no alcanza, los pasos se desplazan (la tabla conserva un mínimo) y los botones quedan a la vista. */}
       <div className="flex min-h-0 flex-1 flex-col gap-2.5 overflow-y-auto pr-1 [@media(min-height:620px)]:overflow-y-hidden">
        <Paso numero={1} titulo="¿A quién le compraste?">
          <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_14rem_14rem]">
            <div>
              <span className="text-sm text-tenue">Proveedor</span>
              <div className="mt-1">
                <CampoProveedor valor={proveedor} onElegir={setProveedor} />
              </div>
            </div>
            <label className="block">
              <span className="text-sm text-tenue">N° de factura o guía (opcional)</span>
              <input value={documento} onChange={(e) => setDocumento(e.target.value)} maxLength={40} placeholder="Por ejemplo, F-1234" className={`mt-1 ${claseCampo}`} />
            </label>
            <label className="block">
              <span className="text-sm text-tenue">Nota (opcional)</span>
              <input value={nota} onChange={(e) => setNota(e.target.value)} maxLength={200} className={`mt-1 ${claseCampo}`} />
            </label>
          </div>
        </Paso>

        <Paso numero={2} titulo="Escanea o busca los productos que llegaron" className="min-h-[13rem] flex-1">
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void buscar();
            }}
            className="relative"
          >
            <input
              ref={escaner}
              autoFocus
              value={entrada}
              onChange={(e) => setEntrada(e.target.value)}
              placeholder="Escanea o escribe el nombre y aprieta Enter"
              aria-label="Agregar producto a la compra"
              autoComplete="off"
              className="h-11 w-full rounded-lg border-2 border-acento bg-superficie px-4 text-base"
            />
            {resultados.length > 0 && (
              <ul className="absolute z-10 mt-1 w-full overflow-hidden tarjeta shadow-s2">
                {resultados.map((p) => (
                  <li key={p.id}>
                    <button type="button" onClick={() => agregar(p)} className="h-11 w-full px-4 text-left hover:bg-suave">
                      {p.nombre}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </form>
          {sinProducto && (
            <div className="mt-2 flex flex-wrap items-center gap-3 rounded-lg bg-aviso-fondo px-3 py-2 text-aviso">
              <span>{sinProducto.texto}</span>
              <Boton
                tamano="chico"
                onClick={() => {
                  setCreando({ codigo: sinProducto.codigo, nombre: sinProducto.nombre });
                  setSinProducto(null);
                }}
              >
                Crear producto
              </Boton>
            </div>
          )}
          <Tabla
            etiqueta="Productos de la compra"
            className="mt-3 min-h-0 flex-1"
            columnas={columnas}
            filas={leidas}
            clave={(x) => x.l.producto.id}
            irAlFinal={irAlFinal}
            vacio={{ texto: "Todavía no agregaste productos. Escanéalos o escribe su nombre arriba." }}
          />
        </Paso>

        <Paso numero={3} titulo="¿Cómo se pagó?">
          <div className="flex flex-wrap items-stretch gap-3">
            <OpcionPago
              marcada={conCaja}
              deshabilitada={hayTurno === false}
              titulo="Con plata de la caja"
              detalle={hayTurno === false ? "No hay un turno abierto: abre la caja para usar esta opción." : "Sale del turno abierto."}
              onElegir={() => setConCaja(true)}
            />
            <OpcionPago
              marcada={!conCaja}
              titulo="De otra forma"
              detalle="No toca la caja."
              onElegir={() => setConCaja(false)}
            />
            <div className="ml-auto text-right tabular-nums">
              <div className="text-sm font-medium text-tenue">Total</div>
              <div className="text-2xl leading-none font-semibold">{pesos(total)}</div>
            </div>
          </div>
        </Paso>
       </div>

        {error && <p className="rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}
        <div className="flex gap-2">
          <Boton variante="terciario" atajo="Esc" onClick={onCerrar} className="flex-1">
            Cancelar
          </Boton>
          <Boton variante="primario" onClick={() => void registrar()} disabled={enviando || lineas.length === 0 || !completas} className="flex-[2]">
            Registrar compra {lineas.length > 0 && `· ${lineas.length} ${lineas.length === 1 ? "producto" : "productos"}`}
          </Boton>
        </div>
      </div>

      {creando !== null && (
        <CapaProductoNuevo
          onCerrar={() => {
            setCreando(null);
            escaner.current?.focus();
          }}
        >
          <FormularioProducto
            producto={null}
            origen="compra"
            codigoInicial={creando.codigo}
            nombreInicial={creando.nombre}
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
        </CapaProductoNuevo>
      )}
    </Dialogo>
  );
}

/** Un paso de la compra: un número y la pregunta que responde. */
function Paso({ numero, titulo, children, className = "" }: { numero: number; titulo: string; children: ReactNode; className?: string }) {
  return (
    <section className={`flex flex-col ${className}`}>
      <h3 className="mb-2 flex items-center gap-2 font-semibold">
        <span aria-hidden className="flex size-6 items-center justify-center rounded-full bg-seleccion text-sm text-acento">
          {numero}
        </span>
        {titulo}
      </h3>
      {children}
    </section>
  );
}

/** Una de las dos formas de pagar: una tarjeta con su explicación, que se elige entera. */
function OpcionPago({
  marcada,
  deshabilitada,
  titulo,
  detalle,
  onElegir,
}: {
  marcada: boolean;
  deshabilitada?: boolean;
  titulo: string;
  detalle: string;
  onElegir: () => void;
}) {
  return (
    <label
      className={`flex max-w-lg min-w-60 flex-1 items-center gap-3 rounded-lg border-2 px-3 py-2 has-focus-visible:outline-2 has-focus-visible:outline-offset-2 has-focus-visible:outline-acento ${
        marcada ? "border-acento bg-seleccion" : "border-linea"
      } ${deshabilitada ? "cursor-not-allowed opacity-60" : "cursor-pointer hover:border-borde-control"}`}
    >
      <input type="radio" name="pago-compra" checked={marcada} disabled={deshabilitada} onChange={onElegir} className="size-4 shrink-0 accent-acento" />
      <span className="min-w-0">
        <span className="font-medium">{titulo}</span>
        <span className="text-sm text-tenue"> · {detalle}</span>
      </span>
    </label>
  );
}

/**
 * El formulario de producto nuevo, encima de la compra. Es un diálogo para el despachador de
 * atajos: Esc lo cierra a él y no a la compra, aunque el foco no esté en el formulario.
 */
function CapaProductoNuevo({ onCerrar, children }: { onCerrar: () => void; children: ReactNode }) {
  useAtajos("dialogo", { Escape: () => onCerrar() });
  return (
    <div
      className="fixed inset-0 z-30 flex items-center justify-center bg-velo p-4"
      // El Esc del formulario ya lo cerró: no llega al despachador (cerraría otra vez).
      onKeyDown={(e) => e.key === "Escape" && e.stopPropagation()}
    >
      <div role="dialog" aria-modal="true" aria-label="Producto nuevo" className="h-[42rem] max-h-full w-[28rem] tarjeta">
        {children}
      </div>
    </div>
  );
}
