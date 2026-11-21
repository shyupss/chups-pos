import { useCallback, useRef, useState } from "react";
import { api, textoError } from "../api";

/** Una tarea en segundo plano sobre la boleta: imprimir o guardar el PDF. */
export type Tarea =
  | { estado: "trabajando" }
  | { estado: "ok"; texto: string }
  | { estado: "error"; texto: string };

/** Lo que pasó con la boleta de una venta. La venta ya está guardada: nada de esto la deshace. */
export interface SalidaVenta {
  ventaId: number;
  /** null si se reimprime la última venta sin tenerla en pantalla (Ctrl+P al abrir la app). */
  folio: number | null;
  impresion: Tarea | null;
  pdf: Tarea | null;
  rutaPdf: string | null;
  /** Error al abrir el cajón, si lo hubo. */
  errorCajon: string | null;
}

const pendiente = (s: SalidaVenta) =>
  s.impresion?.estado === "trabajando" ||
  s.impresion?.estado === "error" ||
  s.pdf?.estado === "trabajando" ||
  s.pdf?.estado === "error" ||
  s.errorCajon !== null;

/**
 * Impresión, PDF y cajón de la última venta. Todo corre en segundo plano: la caja sigue
 * lista para el siguiente cliente mientras la impresora trabaja.
 * `onImpresion` avisa que conviene volver a leer el estado de la impresora.
 */
export function useSalidasVenta(onImpresion: () => void) {
  const [salida, setSalida] = useState<SalidaVenta | null>(null);
  // Para "Reintentar": la misma impresión que falló (original o copia).
  const ultimoIntento = useRef<{ ventaId: number; copias?: number; copia: boolean } | null>(null);

  const actualizar = useCallback((ventaId: number, cambio: Partial<SalidaVenta>) => {
    setSalida((s) => (s && s.ventaId === ventaId ? { ...s, ...cambio } : s));
  }, []);

  const iniciar = useCallback((ventaId: number, folio: number | null) => {
    setSalida({ ventaId, folio, impresion: null, pdf: null, rutaPdf: null, errorCajon: null });
  }, []);

  /** `copia`: es una reimpresión y la boleta dice "COPIA". */
  const imprimir = useCallback(
    async (ventaId: number, copias?: number, copia = false) => {
      ultimoIntento.current = { ventaId, copias, copia };
      actualizar(ventaId, { impresion: { estado: "trabajando" } });
      try {
        await api.imprimirBoleta(ventaId, copias, copia);
        actualizar(ventaId, { impresion: { estado: "ok", texto: copia ? "Copia impresa" : "Boleta impresa" } });
      } catch (e) {
        actualizar(ventaId, { impresion: { estado: "error", texto: textoError(e) } });
      } finally {
        onImpresion();
      }
    },
    [actualizar, onImpresion],
  );

  const guardarPdf = useCallback(
    async (ventaId: number, abrir: boolean) => {
      actualizar(ventaId, { pdf: { estado: "trabajando" } });
      try {
        const ruta = await api.guardarBoletaPdf(ventaId, abrir);
        actualizar(ventaId, { pdf: { estado: "ok", texto: "PDF guardado" }, rutaPdf: ruta });
      } catch (e) {
        actualizar(ventaId, { pdf: { estado: "error", texto: textoError(e) } });
      }
    },
    [actualizar],
  );

  const abrirCajon = useCallback(
    async (ventaId: number) => {
      try {
        await api.abrirCajon();
      } catch (e) {
        actualizar(ventaId, { errorCajon: textoError(e) });
      }
    },
    [actualizar],
  );

  const reintentar = useCallback(() => {
    const u = ultimoIntento.current;
    if (u) void imprimir(u.ventaId, u.copias, u.copia);
  }, [imprimir]);

  const cerrar = useCallback(() => setSalida(null), []);

  /** Al empezar otra venta: se olvida lo que terminó bien, pero un error sigue a la vista. */
  const descartarSiTermino = useCallback(() => {
    setSalida((s) => (s && !pendiente(s) ? null : s));
  }, []);

  return { salida, iniciar, imprimir, reintentar, guardarPdf, abrirCajon, cerrar, descartarSiTermino };
}
