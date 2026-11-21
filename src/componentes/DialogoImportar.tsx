import { useEffect, useState } from "react";
import {
  api,
  textoError,
  type Apertura,
  type CampoCliente,
  type CampoProducto,
  type FilaClienteImportado,
  type FilaError,
  type FilaProductoImportado,
  type ResultadoImportacion,
  type TipoImportacion,
} from "../api";
import { cantidad, pesos } from "../formato";
import TablaVirtual from "../pantallas/reportes/TablaVirtual";
import Dialogo from "./Dialogo";
import Opciones from "./Opciones";
import { esCancelado, useConPin } from "./PedirPin";

const CAMPOS_PRODUCTO: { id: CampoProducto; nombre: string }[] = [
  { id: "codigo", nombre: "Código de barras" },
  { id: "nombre", nombre: "Nombre" },
  { id: "categoria", nombre: "Categoría" },
  { id: "unidad", nombre: "Unidad (unidad o kg)" },
  { id: "precio", nombre: "Precio" },
  { id: "costo", nombre: "Costo" },
  { id: "stock", nombre: "Stock" },
  { id: "stockMinimo", nombre: "Stock mínimo" },
];

const CAMPOS_CLIENTE: { id: CampoCliente; nombre: string }[] = [
  { id: "nombre", nombre: "Nombre" },
  { id: "telefono", nombre: "Teléfono" },
  { id: "rut", nombre: "RUT" },
  { id: "direccion", nombre: "Dirección" },
  { id: "nota", nombre: "Nota" },
  { id: "limite", nombre: "Límite de crédito" },
];

type Vista =
  | { tipo: "productos"; huella: string; nuevas: FilaProductoImportado[]; actualizan: FilaProductoImportado[]; errores: FilaError[]; sinCambios: number; categoriasNuevas: string[] }
  | { tipo: "clientes"; huella: string; nuevas: FilaClienteImportado[]; actualizan: FilaClienteImportado[]; errores: FilaError[]; sinCambios: number };

type Grupo = "nuevas" | "actualizan" | "errores";

interface Props {
  tipo: TipoImportacion;
  onCerrar: () => void;
  onImportado: (r: ResultadoImportacion) => void;
}

/** Importar productos o clientes desde una planilla: columnas, vista previa y confirmación. */
export default function DialogoImportar({ tipo, onCerrar, onImportado }: Props) {
  const conPin = useConPin();
  const [apertura, setApertura] = useState<Apertura | null>(null);
  const [mapeo, setMapeo] = useState<(string | null)[]>([]);
  const [vista, setVista] = useState<Vista | null>(null);
  const [grupo, setGrupo] = useState<Grupo>("nuevas");
  const [resultado, setResultado] = useState<ResultadoImportacion | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [trabajando, setTrabajando] = useState(false);
  const productos = tipo === "productos";
  const campos = productos ? CAMPOS_PRODUCTO : CAMPOS_CLIENTE;
  const que = productos ? "productos" : "clientes";

  const elegir = async () => {
    setError(null);
    try {
      const ruta = await api.elegirPlanilla();
      if (!ruta) {
        if (!apertura) onCerrar();
        return;
      }
      setTrabajando(true);
      const a = await api.abrirPlanilla(ruta);
      setApertura(a);
      setMapeo(productos ? a.productos : a.clientes);
      setVista(null);
    } catch (e) {
      setError(textoError(e));
    } finally {
      setTrabajando(false);
    }
  };
  useEffect(() => {
    void elegir();
    // Al abrir el diálogo se pide el archivo de una vez.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const previsualizar = async () => {
    if (!apertura) return;
    setTrabajando(true);
    setError(null);
    try {
      const v = productos
        ? { tipo: "productos" as const, ...(await api.previsualizarProductos(apertura.ruta, mapeo as (CampoProducto | null)[])) }
        : { tipo: "clientes" as const, ...(await api.previsualizarClientes(apertura.ruta, mapeo as (CampoCliente | null)[])) };
      setVista(v);
      setGrupo(v.nuevas.length > 0 ? "nuevas" : v.actualizan.length > 0 ? "actualizan" : "errores");
    } catch (e) {
      setError(textoError(e));
    } finally {
      setTrabajando(false);
    }
  };

  const importar = async () => {
    if (!apertura || !vista) return;
    setTrabajando(true);
    setError(null);
    const total = vista.nuevas.length + vista.actualizan.length;
    try {
      const r = await conPin(
        (pin) =>
          productos
            ? api.importarProductos(apertura.ruta, mapeo as (CampoProducto | null)[], vista.huella, pin)
            : api.importarClientes(apertura.ruta, mapeo as (CampoCliente | null)[], vista.huella, pin),
        { accion: `Importar ${total} ${que}` },
      );
      setResultado(r);
      onImportado(r);
    } catch (e) {
      if (!esCancelado(e)) setError(textoError(e));
    } finally {
      setTrabajando(false);
    }
  };

  const asignar = (columna: number, campo: string | null) =>
    setMapeo((m) => m.map((c, i) => (i === columna ? campo : c === campo && campo !== null ? null : c)));

  const titulo = productos ? "Importar productos" : "Importar clientes";
  return (
    <Dialogo titulo={titulo} onCerrar={onCerrar} ancho="w-[60rem]">
      {error && <p className="mt-3 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}
      {!apertura ? (
        <p className="mt-3 text-tenue">{trabajando ? "Leyendo la planilla…" : "Elige una planilla .xlsx, .xls, .ods o .csv."}</p>
      ) : resultado ? (
        <div className="mt-4 space-y-4">
          <p className="rounded-lg bg-exito-fondo px-3 py-3 text-exito">
            Listo: se {resultado.creados === 1 ? "creó 1" : `crearon ${resultado.creados}`} y se{" "}
            {resultado.actualizados === 1 ? "actualizó 1" : `actualizaron ${resultado.actualizados}`} {que}.
            {resultado.categoriasCreadas > 0 && ` Categorías nuevas: ${resultado.categoriasCreadas}.`}
            {resultado.omitidos > 0 && ` Se omitieron ${resultado.omitidos} filas con error.`}
          </p>
          <div className="flex justify-end">
            <button type="button" onClick={onCerrar} className="h-10 rounded-lg bg-acento px-5 font-semibold text-sobre-acento">
              Cerrar
            </button>
          </div>
        </div>
      ) : !vista ? (
        <div className="mt-3 space-y-4">
          <p className="text-tenue">
            <span className="font-medium text-texto">{apertura.ruta.split(/[\\/]/).pop()}</span> · {apertura.filas} filas. Revisa qué
            columna es cada dato; las que no se asignan no se importan.
            {productos ? " Los productos con un código que ya existe se actualizan." : " Un cliente con el mismo RUT (o nombre) se actualiza."}
          </p>
          <div className="max-h-96 overflow-auto rounded-lg border border-linea">
            <table className="w-full text-sm">
              <thead className="sticky top-0 bg-fondo text-left text-xs text-tenue">
                <tr>
                  <th className="px-3 py-2 font-medium">Columna de la planilla</th>
                  <th className="px-3 py-2 font-medium">Ejemplos</th>
                  <th className="px-3 py-2 font-medium">Se importa como</th>
                </tr>
              </thead>
              <tbody>
                {apertura.encabezados.map((h, i) => (
                  <tr key={i} className="border-t border-linea">
                    <td className="px-3 py-2 font-medium">{h || `Columna ${i + 1}`}</td>
                    <td className="max-w-64 truncate px-3 py-2 text-tenue">
                      {apertura.muestra
                        .map((f) => f[i])
                        .filter(Boolean)
                        .slice(0, 3)
                        .join(" · ")}
                    </td>
                    <td className="px-3 py-2">
                      <select
                        aria-label={`Campo de la columna ${h}`}
                        value={mapeo[i] ?? ""}
                        onChange={(e) => asignar(i, e.target.value || null)}
                        className="h-9 w-56 rounded-lg border border-linea bg-superficie px-2"
                      >
                        <option value="">No importar</option>
                        {campos.map((c) => (
                          <option key={c.id} value={c.id}>
                            {c.nombre}
                          </option>
                        ))}
                      </select>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="text-xs text-tenue">
            {productos
              ? "Un producto nuevo necesita nombre y precio. El stock entra como un ajuste de inventario con motivo «Importación». Las categorías que no existen se crean."
              : "Las deudas no se importan: solo nacen de ventas fiadas."}
          </p>
          <div className="flex justify-between gap-2">
            <button type="button" onClick={() => void elegir()} className="h-10 rounded-lg border border-linea px-4 hover:bg-fondo">
              Elegir otro archivo
            </button>
            <button
              type="button"
              onClick={() => void previsualizar()}
              disabled={trabajando}
              className="h-10 rounded-lg bg-acento px-5 font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
            >
              {trabajando ? "Revisando…" : "Ver vista previa"}
            </button>
          </div>
        </div>
      ) : (
        <div className="mt-3 space-y-3">
          <p className="text-tenue">
            Nada se guarda hasta confirmar. Las filas con error se omiten.
            {vista.sinCambios > 0 && ` ${vista.sinCambios} filas no cambian nada.`}
            {vista.tipo === "productos" && vista.categoriasNuevas.length > 0 && ` Categorías nuevas: ${vista.categoriasNuevas.join(", ")}.`}
          </p>
          <Opciones
            nombre="grupo-importacion"
            etiqueta="Qué filas ver"
            opciones={[
              { id: "nuevas", nombre: `Nuevas (${vista.nuevas.length})` },
              { id: "actualizan", nombre: `Actualizan (${vista.actualizan.length})` },
              { id: "errores", nombre: `Con error (${vista.errores.length})` },
            ]}
            valor={grupo}
            onCambiar={setGrupo}
          />
          {grupo === "errores" ? (
            <TablaVirtual
              filas={vista.errores}
              clave={(e) => e.fila}
              vacia="Ninguna fila tiene errores."
              alto={340}
              columnas={[
                { titulo: "Fila", ancho: "4rem", celda: (e) => e.fila },
                { titulo: "Motivo", ancho: "minmax(0,1fr)", celda: (e) => <span className="text-peligro" title={e.motivo}>{e.motivo}</span> },
              ]}
            />
          ) : vista.tipo === "productos" ? (
            <TablaVirtual
              filas={vista[grupo]}
              clave={(f) => f.fila}
              vacia="No hay filas aquí."
              alto={340}
              columnas={[
                { titulo: "Fila", ancho: "3.5rem", celda: (f) => f.fila },
                { titulo: "Código", ancho: "8.5rem", celda: (f) => f.codigo ?? "—" },
                { titulo: "Nombre", ancho: "minmax(0,1fr)", celda: (f) => <span title={f.nombre}>{f.nombre}</span> },
                grupo === "actualizan"
                  ? { titulo: "Cambios", ancho: "minmax(0,1.4fr)", celda: (f) => <span title={f.cambios.join(" · ")}>{f.cambios.join(" · ")}</span> }
                  : { titulo: "Categoría", ancho: "8rem", celda: (f) => f.categoria ?? "—" },
                { titulo: "Precio", ancho: "6rem", derecha: true, celda: (f) => pesos(f.precio) },
                { titulo: "Stock", ancho: "6rem", derecha: true, celda: (f) => (f.stockMil === null ? "—" : cantidad(f.stockMil, f.unidad)) },
              ]}
            />
          ) : (
            <TablaVirtual
              filas={vista[grupo]}
              clave={(f) => f.fila}
              vacia="No hay filas aquí."
              alto={340}
              columnas={[
                { titulo: "Fila", ancho: "3.5rem", celda: (f) => f.fila },
                { titulo: "Nombre", ancho: "minmax(0,1fr)", celda: (f) => f.nombre },
                grupo === "actualizan"
                  ? { titulo: "Cambios", ancho: "minmax(0,1.4fr)", celda: (f) => <span title={f.cambios.join(" · ")}>{f.cambios.join(" · ")}</span> }
                  : { titulo: "Teléfono", ancho: "9rem", celda: (f) => f.telefono ?? "—" },
                { titulo: "RUT", ancho: "8rem", celda: (f) => f.rut ?? "—" },
                { titulo: "Límite", ancho: "7rem", derecha: true, celda: (f) => (f.limiteCredito === null ? "Sin límite" : pesos(f.limiteCredito)) },
              ]}
            />
          )}
          <div className="flex justify-between gap-2">
            <button type="button" onClick={() => setVista(null)} className="h-10 rounded-lg border border-linea px-4 hover:bg-fondo">
              Volver a las columnas
            </button>
            <button
              type="button"
              onClick={() => void importar()}
              disabled={trabajando || vista.nuevas.length + vista.actualizan.length === 0}
              className="h-10 rounded-lg bg-acento px-5 font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
            >
              {trabajando ? "Importando…" : `Importar ${vista.nuevas.length + vista.actualizan.length} ${que}`}
            </button>
          </div>
        </div>
      )}
    </Dialogo>
  );
}
