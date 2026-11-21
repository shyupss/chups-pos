import { Ban, FileDown, Printer, Undo2, X } from "lucide-react";
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { api, textoError, type Config, type DetalleVenta, type LineaDetalle, type MedioPago } from "../api";
import { cantidad, fechaHora, folio, NOMBRE_MEDIO, pesos, pesosConSigno } from "../formato";
import { avisarCambioStock } from "../eventos";
import { useSalidasVenta } from "../hooks/useSalidasVenta";
import AvisoVenta from "./AvisoVenta";
import DialogoAnular from "./DialogoAnular";
import DialogoDevolucion from "./DialogoDevolucion";
import InsigniaEstado from "./InsigniaEstado";

interface Props {
  ventaId: number;
  /** La pantalla Ventas está a la vista (sus atajos actúan). */
  activa: boolean;
  onImpresion: () => void;
  /** La venta cambió (anulación o devolución): la lista vuelve a cargar. */
  onCambio: () => void;
  onCerrar: () => void;
}

/** "3 x $1.190" o "0,532 kg x $2.300/kg". */
export const detalleLinea = (l: Pick<LineaDetalle, "cantidadMil" | "unidad" | "precioUnitario">) =>
  `${cantidad(l.cantidadMil, l.unidad)} x ${pesos(l.precioUnitario)}${l.unidad === "kg" ? "/kg" : ""}`;

/** Impresión de un comprobante (anulación o devolución) que se hace al terminar la acción. */
interface Comprobante {
  que: string;
  estado: "trabajando" | "ok" | "error";
  texto?: string;
  imprimir: () => Promise<void>;
}

/** Por qué no se puede anular, o null si se puede. */
function motivoSinAnular(v: DetalleVenta): string | null {
  if (v.estado === "anulada") return "La venta ya está anulada";
  if (v.estado === "conDevolucion" || v.estado === "devuelta")
    return "Tiene devoluciones: devuelve el resto de los productos";
  if (v.estado === "fiada" && v.tieneAbonos) return "La venta fiada tiene abonos: todavía no se puede anular";
  return null;
}

/** Por qué no se puede devolver, o null si se puede. */
function motivoSinDevolver(v: DetalleVenta): string | null {
  if (v.estado === "anulada") return "La venta está anulada";
  if (v.estado === "devuelta") return "Ya se devolvió todo lo vendido";
  if (v.medioPago === "fiado" && v.tieneAbonos) return "La venta fiada tiene abonos: todavía no se puede devolver";
  return null;
}

const reembolso = (m: MedioPago | null) => (m === null ? "sin reembolso (baja la deuda)" : NOMBRE_MEDIO[m].toLowerCase());

/** Detalle de una venta del historial: líneas, pago, turno, anulación y devoluciones. */
export default function DetalleVentaPanel({ ventaId, activa, onImpresion, onCambio, onCerrar }: Props) {
  const [venta, setVenta] = useState<DetalleVenta | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [config, setConfig] = useState<Config | null>(null);
  const [dialogo, setDialogo] = useState<"anular" | "devolver" | null>(null);
  const [comprobante, setComprobante] = useState<Comprobante | null>(null);

  useEffect(() => {
    api
      .obtenerConfig()
      .then(setConfig)
      .catch(() => {});
  }, []);

  /** Imprime el comprobante en segundo plano; si falla, queda el aviso con "Reintentar". */
  const imprimirComprobante = useCallback(
    (que: string, enviar: () => Promise<void>) => {
      const imprimir = async () => {
        setComprobante({ que, estado: "trabajando", imprimir });
        try {
          await enviar();
          setComprobante({ que, estado: "ok", texto: `Comprobante de ${que} impreso`, imprimir });
        } catch (e) {
          setComprobante({ que, estado: "error", texto: textoError(e), imprimir });
        } finally {
          onImpresion();
        }
      };
      void imprimir();
    },
    [onImpresion],
  );

  /** Tras anular o devolver: el detalle nuevo, la lista al día y el comprobante si hay impresora. */
  const alCambiar = (d: DetalleVenta, que: string, enviar: () => Promise<void>) => {
    setVenta(d);
    setDialogo(null);
    onCambio();
    avisarCambioStock();
    if (config?.impresora) imprimirComprobante(que, enviar);
  };
  const { salida, iniciar, imprimir, reintentar, guardarPdf, cerrar } = useSalidasVenta(onImpresion);

  const leer = useCallback(async () => {
    try {
      setVenta(await api.detalleVenta(ventaId));
      setError(null);
    } catch (e) {
      setError(textoError(e));
    }
  }, [ventaId]);
  useEffect(() => {
    void leer();
  }, [leer]);

  const preparar = useCallback(() => {
    if (venta && salida?.ventaId !== venta.id) iniciar(venta.id, venta.folio);
  }, [venta, salida, iniciar]);

  /** Reimpresión: una copia que dice "COPIA" (y "ANULADA" si se anuló). */
  const reimprimir = useCallback(() => {
    if (!venta) return;
    preparar();
    void imprimir(venta.id, 1, true);
  }, [venta, preparar, imprimir]);

  // Ctrl+P reimprime; Esc cierra el detalle.
  useEffect(() => {
    if (!activa) return;
    const alPresionar = (e: KeyboardEvent) => {
      if (document.querySelector('[role="dialog"]')) return;
      if (e.ctrlKey && e.key.toLowerCase() === "p") {
        e.preventDefault();
        reimprimir();
      } else if (e.key === "Escape") {
        onCerrar();
      }
    };
    window.addEventListener("keydown", alPresionar);
    return () => window.removeEventListener("keydown", alPresionar);
  }, [activa, reimprimir, onCerrar]);

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-14 shrink-0 items-center gap-3 border-b border-linea px-5">
        <h2 className="font-semibold tabular-nums">Venta {venta ? folio(venta.folio) : "…"}</h2>
        {venta && <InsigniaEstado estado={venta.estado} />}
        <button
          type="button"
          onClick={onCerrar}
          aria-label="Cerrar detalle"
          title="Cerrar (Esc)"
          className="ml-auto flex size-8 items-center justify-center rounded-lg text-tenue hover:bg-fondo hover:text-texto"
        >
          <X size={18} aria-hidden />
        </button>
      </div>

      {error && <p className="m-5 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}
      {!venta ? (
        !error && <p className="p-5 text-tenue">Cargando…</p>
      ) : (
        <>
          <div className="min-h-0 flex-1 space-y-5 overflow-y-auto p-5 tabular-nums">
            <dl className="space-y-1">
              <Par etiqueta="Fecha y hora" valor={fechaHora(venta.fechaHora)} />
              <Par etiqueta="Turno" valor={venta.turnoId !== null ? `N° ${venta.turnoId}` : "Sin turno (anterior a los turnos)"} />
              {venta.cliente && <Par etiqueta="Cliente" valor={venta.cliente} />}
            </dl>

            <Bloque titulo="Productos">
              <ul className="divide-y divide-linea">
                {venta.lineas.map((l) => (
                  <li key={l.id} className="py-2">
                    <div className="flex items-baseline justify-between gap-3">
                      <span className="min-w-0">{l.nombre}</span>
                      <span className="shrink-0 font-medium">{pesos(l.subtotal)}</span>
                    </div>
                    <div className="flex items-baseline justify-between gap-3 text-xs text-tenue">
                      <span>{detalleLinea(l)}</span>
                      {l.devueltoMil > 0 && (
                        <span className="font-medium text-aviso">
                          Devuelto: {cantidad(l.devueltoMil, l.unidad)}
                          {l.devueltoMil >= l.cantidadMil ? " (todo)" : ""}
                        </span>
                      )}
                    </div>
                  </li>
                ))}
              </ul>
            </Bloque>

            <Bloque titulo="Totales y pago">
              <dl className="space-y-1">
                <Par etiqueta="Subtotal" valor={pesos(venta.subtotal)} />
                {venta.descuento > 0 && <Par etiqueta="Descuento" valor={pesosConSigno(-venta.descuento)} />}
                {venta.redondeo !== 0 && <Par etiqueta="Redondeo" valor={pesosConSigno(venta.redondeo)} />}
                <Par etiqueta="Total" valor={pesos(venta.total)} fuerte />
                <Par etiqueta="Pago" valor={NOMBRE_MEDIO[venta.medioPago]} />
                {venta.medioPago === "efectivo" && venta.montoRecibido !== null && (
                  <>
                    <Par etiqueta="Recibido" valor={pesos(venta.montoRecibido)} />
                    <Par etiqueta="Vuelto" valor={pesos(venta.vuelto ?? 0)} />
                  </>
                )}
                {venta.montoDevuelto > 0 && <Par etiqueta="Devuelto" valor={pesosConSigno(-venta.montoDevuelto)} />}
              </dl>
            </Bloque>

            {venta.anulacion && (
              <Bloque titulo="Anulación">
                <div className="rounded-lg bg-peligro/10 p-3 text-peligro">
                  <div className="font-medium">Anulada el {fechaHora(venta.anulacion.fecha)}</div>
                  <div className="mt-1">Motivo: {venta.anulacion.motivo}</div>
                  <div className="mt-1">
                    Reembolso:{" "}
                    {venta.anulacion.reembolso !== null
                      ? `${pesos(venta.anulacion.reembolso)} en ${reembolso(venta.anulacion.medioReembolso)}`
                      : reembolso(null)}
                    {venta.anulacion.turnoId !== null && ` · turno N° ${venta.anulacion.turnoId}`}
                  </div>
                </div>
              </Bloque>
            )}

            {venta.devoluciones.length > 0 && (
              <Bloque titulo="Devoluciones">
                <ul className="space-y-3">
                  {venta.devoluciones.map((d) => (
                    <li key={d.id} className="rounded-lg border border-linea p-3">
                      <div className="flex items-baseline justify-between gap-3">
                        <span className="font-medium">{fechaHora(d.fecha)}</span>
                        <span className="font-semibold">{pesosConSigno(-d.monto)}</span>
                      </div>
                      <div className="text-xs text-tenue">
                        {d.medioReembolso ? `Reembolso en ${reembolso(d.medioReembolso)}` : "Sin reembolso: baja la deuda"}
                        {d.redondeo !== 0 && ` · redondeo ${pesosConSigno(d.redondeo)}`}
                        {d.turnoId !== null && ` · turno N° ${d.turnoId}`}
                      </div>
                      <ul className="mt-2 space-y-0.5">
                        {d.lineas.map((l) => (
                          <li key={l.ventaItemId} className="flex items-baseline justify-between gap-3">
                            <span className="min-w-0">
                              {cantidad(l.cantidadMil, l.unidad)} · {l.nombre}
                              {!l.vuelveAStock && <span className="text-xs text-aviso"> (merma, no volvió al stock)</span>}
                            </span>
                            <span className="shrink-0 text-tenue">{pesos(l.monto)}</span>
                          </li>
                        ))}
                      </ul>
                      <div className="mt-2 text-xs text-tenue">Motivo: {d.motivo}</div>
                    </li>
                  ))}
                </ul>
              </Bloque>
            )}
          </div>

          <div className="shrink-0 space-y-2 border-t border-linea p-4">
            {comprobante && (
              <div
                role={comprobante.estado === "error" ? "alert" : "status"}
                className={`flex flex-wrap items-center gap-3 rounded-lg px-3 py-2 ${
                  comprobante.estado === "error" ? "bg-peligro/10 text-peligro" : "bg-fondo text-tenue"
                }`}
              >
                <span className="min-w-0 flex-1">
                  {comprobante.estado === "trabajando"
                    ? `Imprimiendo el comprobante de ${comprobante.que}…`
                    : comprobante.estado === "error"
                      ? `No se imprimió el comprobante de ${comprobante.que}: ${comprobante.texto}`
                      : comprobante.texto}
                </span>
                {comprobante.estado === "error" && (
                  <button
                    type="button"
                    onClick={() => void comprobante.imprimir()}
                    className="h-8 rounded-lg bg-peligro px-3 font-medium text-superficie"
                  >
                    Reintentar
                  </button>
                )}
              </div>
            )}
            {salida && (
              <AvisoVenta
                venta={null}
                salida={salida}
                reimprimir={reimprimir}
                reintentar={reintentar}
                guardarPdf={() => {
                  preparar();
                  void guardarPdf(venta.id, false);
                }}
                abrirPdf={() => void guardarPdf(venta.id, true)}
                cerrar={cerrar}
              />
            )}
            <div className="flex flex-wrap gap-2">
              <Boton onClick={reimprimir} titulo="Reimprimir (Ctrl+P)" icono={<Printer size={16} aria-hidden />}>
                Reimprimir <span className="text-xs text-tenue">Ctrl+P</span>
              </Boton>
              <Boton
                onClick={() => {
                  preparar();
                  void guardarPdf(venta.id, false);
                }}
                icono={<FileDown size={16} aria-hidden />}
              >
                Guardar PDF
              </Boton>
              <Boton
                onClick={() => setDialogo("devolver")}
                disabled={motivoSinDevolver(venta) !== null}
                titulo={motivoSinDevolver(venta) ?? "Devolver parte de lo vendido"}
                icono={<Undo2 size={16} aria-hidden />}
              >
                Devolver productos
              </Boton>
              <Boton
                onClick={() => setDialogo("anular")}
                disabled={motivoSinAnular(venta) !== null}
                titulo={motivoSinAnular(venta) ?? "Repone el stock y devuelve el dinero"}
                icono={<Ban size={16} aria-hidden />}
                peligro
              >
                Anular venta
              </Boton>
            </div>
          </div>
          {dialogo === "devolver" && activa && (
            <DialogoDevolucion
              venta={venta}
              redondeoActivo={config?.redondeoEfectivo ?? true}
              onCerrar={() => setDialogo(null)}
              onDevuelta={(r) => alCambiar(r.venta, "devolución", () => api.imprimirDevolucion(r.devolucionId))}
            />
          )}
          {dialogo === "anular" && activa && (
            <DialogoAnular
              venta={venta}
              redondeoActivo={config?.redondeoEfectivo ?? true}
              onCerrar={() => setDialogo(null)}
              onAnulada={(d) => alCambiar(d, "anulación", () => api.imprimirAnulacion(d.id))}
            />
          )}
        </>
      )}
    </div>
  );
}

function Bloque({ titulo, children }: { titulo: string; children: ReactNode }) {
  return (
    <section>
      <h3 className="mb-1 text-xs font-medium uppercase tracking-wide text-tenue">{titulo}</h3>
      {children}
    </section>
  );
}

function Par({ etiqueta, valor, fuerte }: { etiqueta: string; valor: string; fuerte?: boolean }) {
  return (
    <div className={`flex items-baseline justify-between gap-3 ${fuerte ? "text-lg font-semibold" : ""}`}>
      <dt className={fuerte ? "" : "text-tenue"}>{etiqueta}</dt>
      <dd className="text-right">{valor}</dd>
    </div>
  );
}

export function Boton({
  onClick,
  children,
  icono,
  titulo,
  peligro,
  disabled,
}: {
  onClick: () => void;
  children: ReactNode;
  icono?: ReactNode;
  titulo?: string;
  peligro?: boolean;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={titulo}
      disabled={disabled}
      className={`flex h-10 items-center gap-2 rounded-lg border px-3 disabled:opacity-50 ${
        peligro
          ? "border-peligro/40 text-peligro enabled:hover:bg-peligro/10"
          : "border-linea enabled:hover:bg-fondo"
      }`}
    >
      {icono}
      {children}
    </button>
  );
}
