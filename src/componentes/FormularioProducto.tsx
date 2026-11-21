import { useEffect, useRef, useState } from "react";
import { api, textoError, type Categoria, type Producto, type Unidad } from "../api";
import { aEntero, cantidad, cantidadEditable, parseCantidad, pesos, soloDigitos } from "../formato";
import Opciones, { type Opcion } from "./Opciones";

interface Props {
  /** null = producto nuevo */
  producto: Producto | null;
  codigoInicial?: string;
  categorias: Categoria[];
  /** Categoría para un producto nuevo (la del filtro de la lista, si hay una). */
  categoriaInicial: number | null;
  onGuardado: (p: Producto) => void;
  /** Abre su historial de stock en Inventario > Movimientos. */
  onVerMovimientos?: (p: Producto) => void;
  onCerrar: () => void;
}

const UNIDADES: Opcion<Unidad>[] = [
  { id: "unidad", nombre: "Unidad" },
  { id: "kg", nombre: "Kilo" },
];

export default function FormularioProducto({
  producto,
  codigoInicial,
  categorias,
  categoriaInicial,
  onGuardado,
  onVerMovimientos,
  onCerrar,
}: Props) {
  const nuevo = producto === null;
  const [codigo, setCodigo] = useState(producto?.codigoBarras ?? codigoInicial ?? "");
  const [nombre, setNombre] = useState(producto?.nombre ?? "");
  const [categoriaId, setCategoriaId] = useState<number | null>(producto ? producto.categoriaId : categoriaInicial);
  const [unidad, setUnidad] = useState<Unidad>(producto?.unidad ?? "unidad");
  const [precio, setPrecio] = useState(producto ? String(producto.precioVenta) : "");
  const [costo, setCosto] = useState(producto?.costo != null ? String(producto.costo) : "");
  const [stockMinimo, setStockMinimo] = useState(
    producto ? cantidadEditable(producto.stockMinimoMil, producto.unidad) : "0",
  );
  const [stockInicial, setStockInicial] = useState("0");
  const [error, setError] = useState<string | null>(null);
  const [guardando, setGuardando] = useState(false);
  const primerCampo = useRef<HTMLInputElement>(null);

  const porKg = unidad === "kg";
  const enUnidad = porKg ? " (kg)" : "";
  // En kg se aceptan decimales ("1,5"); en unidades, solo dígitos.
  const limpiarStock = (texto: string) => (porKg ? texto.replace(/[^\d.,]/g, "") : soloDigitos(texto));

  useEffect(() => {
    primerCampo.current?.focus();
  }, []);

  const guardar = async () => {
    const precioVenta = aEntero(precio);
    if (precioVenta === null) return setError(porKg ? "Ingresa el precio por kg" : "Ingresa el precio de venta");
    const stockMinimoMil = parseCantidad(stockMinimo || "0", unidad);
    const stockInicialMil = nuevo ? parseCantidad(stockInicial || "0", unidad) : 0;
    if (stockMinimoMil === null || stockInicialMil === null) {
      return setError(
        porKg
          ? "El stock va en kg, con hasta 3 decimales (por ejemplo 12,5)"
          : "El stock va en unidades enteras",
      );
    }
    setGuardando(true);
    setError(null);
    const datos = {
      codigoBarras: codigo.trim() || null,
      nombre,
      categoriaId,
      unidad,
      precioVenta,
      costo: aEntero(costo),
      stockMinimoMil,
      stockInicialMil,
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
      className="flex h-full flex-col gap-4 overflow-y-auto p-6"
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
      <Campo etiqueta="Categoría">
        <select
          value={categoriaId ?? ""}
          onChange={(e) => setCategoriaId(e.target.value === "" ? null : Number(e.target.value))}
          className={claseCampo}
        >
          <option value="">Sin categoría</option>
          {categorias.map((c) => (
            <option key={c.id} value={c.id}>
              {c.nombre}
            </option>
          ))}
        </select>
      </Campo>
      <div>
        <span className="text-xs text-tenue">Se vende por</span>
        <div className="mt-1">
          <Opciones nombre="unidad" etiqueta="Se vende por" opciones={UNIDADES} valor={unidad} onCambiar={setUnidad} />
        </div>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <Campo etiqueta={porKg ? "Precio por kg" : "Precio de venta"}>
          <input
            value={precio ? pesos(Number(soloDigitos(precio))) : ""}
            onChange={(e) => setPrecio(soloDigitos(e.target.value))}
            className={claseCampo}
            inputMode="numeric"
          />
        </Campo>
        <Campo etiqueta={porKg ? "Costo por kg (opcional)" : "Costo (opcional)"}>
          <input
            value={costo ? pesos(Number(soloDigitos(costo))) : ""}
            onChange={(e) => setCosto(soloDigitos(e.target.value))}
            className={claseCampo}
            inputMode="numeric"
          />
        </Campo>
        {nuevo ? (
          <Campo etiqueta={`Stock inicial${enUnidad}`}>
            <input
              value={stockInicial}
              onChange={(e) => setStockInicial(limpiarStock(e.target.value))}
              className={claseCampo}
              inputMode="decimal"
            />
          </Campo>
        ) : (
          <Campo etiqueta="Stock actual">
            <div className="flex h-10 items-center tabular-nums">{cantidad(producto.stockMil, unidad)}</div>
          </Campo>
        )}
        <Campo etiqueta={`Avisar con stock menor o igual a${enUnidad}`}>
          <input
            value={stockMinimo}
            onChange={(e) => setStockMinimo(limpiarStock(e.target.value))}
            className={claseCampo}
            inputMode="decimal"
          />
        </Campo>
      </div>

      {error && <p className="rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}

      <div className="mt-auto flex flex-wrap gap-2">
        <button
          type="submit"
          disabled={guardando}
          className="h-11 flex-1 rounded-lg bg-acento font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
        >
          Guardar
        </button>
        <button type="button" onClick={onCerrar} className="h-11 rounded-lg px-4 text-tenue hover:bg-fondo hover:text-texto">
          Cerrar
        </button>
        {!nuevo && onVerMovimientos && (
          <button
            type="button"
            onClick={() => onVerMovimientos(producto)}
            className="h-10 w-full rounded-lg border border-linea hover:bg-fondo"
          >
            Ver movimientos
          </button>
        )}
        {!nuevo && (
          <button
            type="button"
            onClick={() => void alternarActivo()}
            className="h-10 w-full rounded-lg text-tenue hover:bg-fondo hover:text-texto"
          >
            {producto.activo ? "Desactivar (deja de aparecer en la caja)" : "Volver a activar"}
          </button>
        )}
      </div>
    </form>
  );
}

const claseCampo =
  "h-10 w-full rounded-lg border border-linea bg-fondo px-3 tabular-nums outline-none focus:border-texto";

function Campo({ etiqueta, children }: { etiqueta: string; children: React.ReactNode }) {
  return (
    <label className="block">
      <span className="text-xs text-tenue">{etiqueta}</span>
      <div className="mt-1">{children}</div>
    </label>
  );
}
