import { useRef, useState } from "react";
import { api, textoError, type Diferencia, type Producto } from "../api";
import { cantidad, pesos, pesosConSigno } from "../formato";
import CampoCantidad from "./CampoCantidad";
import Dialogo from "./Dialogo";
import { esCancelado, useConPin } from "./PedirPin";

interface Linea {
  producto: Producto;
  contadoMil: number;
}

interface Props {
  onTerminado: (aplicadas: Diferencia[]) => void;
  onCerrar: () => void;
}

/**
 * Conteo de varios productos seguidos: escanear (o buscar), escribir lo contado, Enter y el
 * siguiente. Al terminar, Rust calcula las diferencias contra el stock y se confirman todas
 * juntas en una transacción.
 */
export default function ConteoInventario({ onTerminado, onCerrar }: Props) {
  const conPin = useConPin();
  const [lineas, setLineas] = useState<Linea[]>([]);
  const [entrada, setEntrada] = useState("");
  const [resultados, setResultados] = useState<Producto[]>([]);
  const [actual, setActual] = useState<{ producto: Producto; mil: number | null } | null>(null);
  const [resumen, setResumen] = useState<Diferencia[] | null>(null);
  const [motivo, setMotivo] = useState("Conteo de inventario");
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);
  const escaner = useRef<HTMLInputElement>(null);

  const elegir = (p: Producto) => {
    setResultados([]);
    setError(null);
    // Si ya estaba, se corrige su cantidad.
    const previa = lineas.find((l) => l.producto.id === p.id);
    setActual({ producto: p, mil: previa?.contadoMil ?? null });
  };

  const buscar = async () => {
    const texto = entrada.trim();
    setEntrada("");
    if (!texto) return;
    try {
      const porCodigo = /^[A-Za-z0-9-]+$/.test(texto) ? await api.buscarProductoPorCodigo(texto) : null;
      if (porCodigo) return elegir(porCodigo);
      const encontrados = await api.buscarProductos(texto, false, 8, 0);
      if (encontrados.length === 1) return elegir(encontrados[0]);
      if (encontrados.length === 0) return setError(`No hay productos que coincidan con "${texto}"`);
      setResultados(encontrados);
    } catch (e) {
      setError(textoError(e));
    }
  };

  const agregarActual = (mil = actual?.mil ?? null) => {
    if (!actual || mil === null) return;
    const p = actual.producto;
    setLineas((ls) => {
      const sin = ls.filter((l) => l.producto.id !== p.id);
      return [{ producto: p, contadoMil: mil }, ...sin];
    });
    setActual(null);
    setTimeout(() => escaner.current?.focus(), 0);
  };

  const revisar = async () => {
    if (lineas.length === 0) return;
    setError(null);
    try {
      setResumen(await api.previsualizarConteo(lineas.map((l) => ({ productoId: l.producto.id, contadoMil: l.contadoMil }))));
    } catch (e) {
      setError(textoError(e));
    }
  };

  const confirmar = async () => {
    if (!resumen || enviando || !motivo.trim()) return;
    setEnviando(true);
    setError(null);
    try {
      const aplicadas = await conPin(
        (pin) =>
          api.ajustarConteo(
            lineas.map((l) => ({ productoId: l.producto.id, contadoMil: l.contadoMil })),
            motivo,
            pin,
          ),
        { accion: "Ajustar el inventario contado" },
      );
      onTerminado(aplicadas);
    } catch (e) {
      if (!esCancelado(e)) setError(textoError(e));
      setEnviando(false);
    }
  };

  if (resumen) {
    const conDiferencia = resumen.filter((d) => d.diferenciaMil !== 0);
    const valorNeto = resumen.reduce((s, d) => s + (d.valor ?? 0), 0);
    return (
      <Dialogo titulo="Conteo · resumen de diferencias" onCerrar={() => setResumen(null)} ancho="w-[46rem]">
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void confirmar();
          }}
          className="mt-4"
        >
          <p className="text-tenue">
            {resumen.length} productos contados, {conDiferencia.length} con diferencia. Al confirmar, el stock de cada uno
            queda en lo contado (la diferencia se recalcula con el stock de ese momento).
          </p>
          <div className="mt-3 max-h-80 overflow-auto rounded-lg border border-linea">
            <table className="w-full">
              <thead className="sticky top-0 bg-superficie text-left text-xs uppercase text-tenue">
                <tr className="h-9">
                  <th className="px-3 font-medium">Producto</th>
                  <th className="px-3 text-right font-medium">Sistema</th>
                  <th className="px-3 text-right font-medium">Contado</th>
                  <th className="px-3 text-right font-medium">Diferencia</th>
                  <th className="px-3 text-right font-medium">A costo</th>
                </tr>
              </thead>
              <tbody>
                {resumen.map((d) => (
                  <tr key={d.productoId} className="h-9 border-t border-linea">
                    <td className="max-w-60 truncate px-3">{d.nombre}</td>
                    <td className="px-3 text-right">{cantidad(d.stockMil, d.unidad)}</td>
                    <td className="px-3 text-right">{cantidad(d.contadoMil, d.unidad)}</td>
                    <td
                      className={`px-3 text-right font-medium ${
                        d.diferenciaMil > 0 ? "text-aviso" : d.diferenciaMil < 0 ? "text-peligro" : "text-tenue"
                      }`}
                    >
                      {d.diferenciaMil === 0
                        ? "—"
                        : `${d.diferenciaMil > 0 ? "+" : "−"}${cantidad(Math.abs(d.diferenciaMil), d.unidad)}`}
                    </td>
                    <td className="px-3 text-right text-tenue">
                      {d.valor === null ? "sin costo" : d.valor === 0 ? "—" : pesosConSigno(d.valor)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div className="mt-3 flex items-baseline justify-between tabular-nums">
            <span className="text-tenue">Diferencia total a costo</span>
            <span className={`text-xl font-semibold ${valorNeto < 0 ? "text-peligro" : ""}`}>
              {valorNeto === 0 ? pesos(0) : pesosConSigno(valorNeto)}
            </span>
          </div>
          <label className="mt-4 block">
            <span className="text-tenue">Motivo (obligatorio)</span>
            <input
              autoFocus
              value={motivo}
              onChange={(e) => setMotivo(e.target.value)}
              maxLength={200}
              className="mt-1 h-10 w-full rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento"
            />
          </label>
          {error && <p className="mt-3 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}
          <div className="mt-5 flex gap-2">
            <button
              type="button"
              onClick={() => setResumen(null)}
              className="h-11 flex-1 rounded-lg text-tenue hover:bg-fondo hover:text-texto"
            >
              Seguir contando <span className="text-xs opacity-60">Esc</span>
            </button>
            <button
              type="submit"
              disabled={enviando || !motivo.trim() || conDiferencia.length === 0}
              className="h-11 flex-[2] rounded-lg bg-acento font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
            >
              {conDiferencia.length === 0 ? "No hay diferencias" : `Ajustar ${conDiferencia.length} productos`}{" "}
              <span className="text-xs font-normal opacity-70">Enter</span>
            </button>
          </div>
        </form>
      </Dialogo>
    );
  }

  return (
    <Dialogo titulo="Contar inventario" onCerrar={onCerrar} ancho="w-[46rem]">
      <p className="mt-1 text-tenue">Escanea o busca un producto, escribe cuánto contaste y Enter. Escanear de nuevo corrige.</p>
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
          disabled={actual !== null}
          placeholder="Escanea o escribe y Enter"
          aria-label="Producto a contar"
          autoComplete="off"
          className="h-11 w-full rounded-lg border-2 border-acento bg-superficie px-4 outline-none disabled:border-linea"
        />
        {resultados.length > 0 && (
          <ul className="absolute z-10 mt-1 w-full overflow-hidden tarjeta shadow-s2">
            {resultados.map((p) => (
              <li key={p.id}>
                <button
                  type="button"
                  onClick={() => elegir(p)}
                  className="flex h-10 w-full items-center justify-between px-4 text-left hover:bg-fondo"
                >
                  <span>{p.nombre}</span>
                  <span className="text-tenue tabular-nums">{cantidad(p.stockMil, p.unidad)}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </form>

      {actual && (
        <div className="mt-3 flex items-center gap-3 rounded-lg border border-acento p-3">
          <div className="min-w-0 flex-1">
            <div className="truncate font-medium">{actual.producto.nombre}</div>
            <div className="text-xs text-tenue">
              {actual.producto.unidad === "kg" ? "Por kg" : "Por unidad"} · Enter para agregar, Esc para cancelar
            </div>
          </div>
          <div className="w-48" onKeyDown={(e) => e.key === "Escape" && (e.stopPropagation(), setActual(null))}>
            <CampoCantidad
              producto={actual.producto}
              valorMil={actual.mil}
              onCambiar={(mil) => {
                if (actual.producto.unidad === "kg") agregarActual(mil);
                else setActual({ ...actual, mil });
              }}
              minimoMil={0}
              autoFocus
              etiqueta={`Cantidad contada de ${actual.producto.nombre}`}
              onEnter={() => agregarActual()}
            />
          </div>
        </div>
      )}

      <div className="mt-3 max-h-72 min-h-24 overflow-auto rounded-lg border border-linea">
        {lineas.length === 0 ? (
          <p className="p-6 text-center text-tenue">Todavía no hay productos contados.</p>
        ) : (
          <ul className="divide-y divide-linea">
            {lineas.map((l) => (
              <li key={l.producto.id} className="flex h-10 items-center gap-3 px-3 tabular-nums">
                <span className="min-w-0 flex-1 truncate">{l.producto.nombre}</span>
                <span className="font-medium">{cantidad(l.contadoMil, l.producto.unidad)}</span>
                <button
                  type="button"
                  onClick={() => setLineas((ls) => ls.filter((x) => x.producto.id !== l.producto.id))}
                  aria-label={`Quitar ${l.producto.nombre}`}
                  className="h-7 rounded-lg px-2 text-tenue hover:bg-peligro/10 hover:text-peligro"
                >
                  ×
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
      {error && <p className="mt-3 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}
      <div className="mt-5 flex gap-2">
        <button type="button" onClick={onCerrar} className="h-11 flex-1 rounded-lg text-tenue hover:bg-fondo hover:text-texto">
          Cancelar <span className="text-xs opacity-60">Esc</span>
        </button>
        <button
          type="button"
          onClick={() => void revisar()}
          disabled={lineas.length === 0}
          className="h-11 flex-[2] rounded-lg bg-acento font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
        >
          Revisar diferencias ({lineas.length})
        </button>
      </div>
    </Dialogo>
  );
}
