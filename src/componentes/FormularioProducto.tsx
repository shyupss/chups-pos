import { useEffect, useRef, useState } from "react";
import { api, textoError, type Producto } from "../api";
import { aEntero, pesos, soloDigitos } from "../formato";

interface Props {
  /** null = producto nuevo */
  producto: Producto | null;
  codigoInicial?: string;
  onGuardado: (p: Producto) => void;
  onCerrar: () => void;
}

export default function FormularioProducto({ producto, codigoInicial, onGuardado, onCerrar }: Props) {
  const nuevo = producto === null;
  const [codigo, setCodigo] = useState(producto?.codigoBarras ?? codigoInicial ?? "");
  const [nombre, setNombre] = useState(producto?.nombre ?? "");
  const [precio, setPrecio] = useState(producto ? String(producto.precioVenta) : "");
  const [costo, setCosto] = useState(producto?.costo != null ? String(producto.costo) : "");
  const [stockMinimo, setStockMinimo] = useState(producto ? String(producto.stockMinimo) : "0");
  const [stockInicial, setStockInicial] = useState("0");
  const [error, setError] = useState<string | null>(null);
  const [guardando, setGuardando] = useState(false);
  const primerCampo = useRef<HTMLInputElement>(null);

  useEffect(() => {
    primerCampo.current?.focus();
  }, []);

  const guardar = async () => {
    const precioVenta = aEntero(precio);
    if (precioVenta === null) return setError("Ingresa el precio de venta");
    setGuardando(true);
    setError(null);
    const datos = {
      codigoBarras: codigo.trim() || null,
      nombre,
      precioVenta,
      costo: aEntero(costo),
      stockMinimo: aEntero(stockMinimo) ?? 0,
      stockInicial: nuevo ? (aEntero(stockInicial) ?? 0) : 0,
    };
    try {
      const guardado = nuevo
        ? await api.crearProducto(datos)
        : await api.editarProducto(producto.id, datos);
      onGuardado(guardado);
    } catch (e) {
      setError(textoError(e));
    } finally {
      setGuardando(false);
    }
  };

  const alternarActivo = async () => {
    if (!producto) return;
    try {
      await api.cambiarEstadoProducto(producto.id, !producto.activo);
      onGuardado({ ...producto, activo: !producto.activo });
    } catch (e) {
      setError(textoError(e));
    }
  };

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        void guardar();
      }}
      onKeyDown={(e) => e.key === "Escape" && onCerrar()}
      className="flex h-full flex-col gap-4 p-6"
    >
      <h2 className="text-lg font-semibold">{nuevo ? "Nuevo producto" : "Editar producto"}</h2>

      <Campo etiqueta="Código de barras (puedes escanearlo aquí)">
        <input
          ref={codigo ? undefined : primerCampo}
          value={codigo}
          onChange={(e) => setCodigo(e.target.value.trim())}
          onKeyDown={(e) => {
            // El lector termina con Enter: aquí solo pasa al siguiente campo.
            if (e.key === "Enter") {
              e.preventDefault();
              (e.currentTarget.form?.elements.namedItem("nombre") as HTMLInputElement | null)?.focus();
            }
          }}
          className={claseCampo}
          autoComplete="off"
        />
      </Campo>
      <Campo etiqueta="Nombre">
        <input
          ref={codigo ? primerCampo : undefined}
          name="nombre"
          value={nombre}
          onChange={(e) => setNombre(e.target.value)}
          className={claseCampo}
          maxLength={120}
          autoComplete="off"
        />
      </Campo>
      <div className="grid grid-cols-2 gap-3">
        <Campo etiqueta="Precio de venta">
          <input
            value={precio ? pesos(Number(soloDigitos(precio))) : ""}
            onChange={(e) => setPrecio(soloDigitos(e.target.value))}
            className={claseCampo}
            inputMode="numeric"
          />
        </Campo>
        <Campo etiqueta="Costo (opcional)">
          <input
            value={costo ? pesos(Number(soloDigitos(costo))) : ""}
            onChange={(e) => setCosto(soloDigitos(e.target.value))}
            className={claseCampo}
            inputMode="numeric"
          />
        </Campo>
        {nuevo ? (
          <Campo etiqueta="Stock inicial">
            <input
              value={stockInicial}
              onChange={(e) => setStockInicial(soloDigitos(e.target.value))}
              className={claseCampo}
              inputMode="numeric"
            />
          </Campo>
        ) : (
          <Campo etiqueta="Stock actual">
            <div className="py-2 text-slate-700">{producto.stock}</div>
          </Campo>
        )}
        <Campo etiqueta="Avisar con stock menor o igual a">
          <input
            value={stockMinimo}
            onChange={(e) => setStockMinimo(soloDigitos(e.target.value))}
            className={claseCampo}
            inputMode="numeric"
          />
        </Campo>
      </div>

      {error && <p className="rounded bg-red-50 px-3 py-2 text-sm text-red-800">{error}</p>}

      <div className="mt-auto flex flex-wrap gap-2">
        <button
          type="submit"
          disabled={guardando}
          className="flex-1 rounded-lg bg-slate-900 py-3 font-semibold text-white disabled:bg-slate-300"
        >
          Guardar
        </button>
        <button type="button" onClick={onCerrar} className="rounded-lg px-4 text-slate-600 hover:bg-slate-100">
          Cerrar
        </button>
        {!nuevo && (
          <button
            type="button"
            onClick={() => void alternarActivo()}
            className="w-full rounded-lg py-2 text-sm text-slate-500 hover:bg-slate-100"
          >
            {producto.activo ? "Desactivar (deja de aparecer en la caja)" : "Volver a activar"}
          </button>
        )}
      </div>
    </form>
  );
}

const claseCampo =
  "w-full rounded-lg border border-slate-300 px-3 py-2 outline-none focus:border-slate-900";

function Campo({ etiqueta, children }: { etiqueta: string; children: React.ReactNode }) {
  return (
    <label className="block">
      <span className="text-xs text-slate-500">{etiqueta}</span>
      <div className="mt-1">{children}</div>
    </label>
  );
}
