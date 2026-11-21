import { FileDown, Pencil, Printer } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { api, textoError, type Config, type FichaCliente as Ficha, type Pago, type VentaFiada } from "../../api";
import Dialogo from "../../componentes/Dialogo";
import { esCancelado, useConPin } from "../../componentes/PedirPin";
import RegistrarPago from "../../componentes/RegistrarPago";
import { fechaHora, folio, NOMBRE_MEDIO, pesos } from "../../formato";

interface Props {
  clienteId: number;
  activa: boolean;
  config: Config | null;
  onCambio: () => void;
  onEditar: () => void;
  onIrAVenta: (folio: number) => void;
  onImpresion: () => void;
}

type Aviso = { texto: string; tipo: "ok" | "error" | "trabajando"; reintentar?: () => void };

/** Días desde una fecha ISO hasta hoy. */
export const diasDesde = (iso: string) => Math.floor((Date.now() - new Date(iso).getTime()) / 86_400_000);

/** Ficha del cliente: deuda, ventas pendientes (para marcar), pagos y estado de cuenta. */
export default function FichaCliente({ clienteId, activa, config, onCambio, onEditar, onIrAVenta, onImpresion }: Props) {
  const conPin = useConPin();
  const [ficha, setFicha] = useState<Ficha | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [marcadas, setMarcadas] = useState<Set<number>>(new Set());
  const [pagando, setPagando] = useState(false);
  const [anulando, setAnulando] = useState<Pago | null>(null);
  const [aviso, setAviso] = useState<Aviso | null>(null);
  /** Lo último que pasó (pago registrado o anulado), aparte del aviso de impresión. */
  const [hecho, setHecho] = useState<string | null>(null);

  const leer = useCallback(async () => {
    try {
      setFicha(await api.fichaCliente(clienteId));
      setError(null);
    } catch (e) {
      setError(textoError(e));
    }
  }, [clienteId]);
  useEffect(() => {
    setMarcadas(new Set());
    setAviso(null);
    setHecho(null);
    void leer();
  }, [leer]);

  const deuda = ficha?.cliente.deuda ?? 0;
  useEffect(() => {
    if (!activa) return;
    const alPresionar = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key.toLowerCase() === "p") {
        // Ctrl+P es "registrar pago", no imprimir la ventana.
        e.preventDefault();
        if (!document.querySelector('[role="dialog"]') && deuda > 0) setPagando(true);
      }
    };
    window.addEventListener("keydown", alPresionar);
    return () => window.removeEventListener("keydown", alPresionar);
  }, [activa, deuda]);

  /** Imprime en segundo plano y deja el resultado a la vista, con "Reintentar" si falla. */
  const imprimir = useCallback(
    (que: string, enviar: () => Promise<void>) => {
      const intentar = async () => {
        setAviso({ texto: `Imprimiendo ${que}…`, tipo: "trabajando" });
        try {
          await enviar();
          setAviso({ texto: `${que[0].toUpperCase()}${que.slice(1)} impreso`, tipo: "ok" });
        } catch (e) {
          setAviso({ texto: textoError(e), tipo: "error", reintentar: () => void intentar() });
        } finally {
          onImpresion();
        }
      };
      void intentar();
    },
    [onImpresion],
  );

  const pdf = async () => {
    setAviso({ texto: "Guardando el PDF…", tipo: "trabajando" });
    try {
      const ruta = await api.guardarEstadoCuentaPdf(clienteId, true);
      setAviso({ texto: `PDF guardado en ${ruta}`, tipo: "ok" });
    } catch (e) {
      setAviso({ texto: textoError(e), tipo: "error" });
    }
  };

  const cambiarActivo = async () => {
    if (!ficha) return;
    try {
      await api.cambiarEstadoCliente(clienteId, !ficha.cliente.activo);
      await leer();
      onCambio();
    } catch (e) {
      setAviso({ texto: textoError(e), tipo: "error" });
    }
  };

  if (error) return <p className="m-5 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>;
  if (!ficha) return <p className="p-5 text-tenue">Cargando…</p>;
  const { cliente, pendientes, pagadas, pagos } = ficha;
  const elegidas = pendientes.filter((v) => marcadas.has(v.ventaId));
  const alternar = (id: number) =>
    setMarcadas((m) => {
      const n = new Set(m);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  const diasAlerta = config?.fiadoDiasAlerta ?? 30;

  return (
    <div className="flex h-full flex-col overflow-auto p-5">
      <header className="flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0">
          <h2 className="truncate text-xl font-semibold">
            {cliente.nombre}
            {!cliente.activo && <span className="ml-2 text-sm font-normal text-tenue">(desactivado)</span>}
          </h2>
          <p className="text-tenue">
            {[cliente.telefono, cliente.rut && `RUT ${cliente.rut}`, cliente.direccion].filter(Boolean).join(" · ") || "Sin datos de contacto"}
          </p>
          {cliente.nota && <p className="mt-1 text-tenue italic">{cliente.nota}</p>}
        </div>
        <div className="text-right tabular-nums">
          <div className="text-tenue">Debe</div>
          <div className="text-3xl font-semibold">{pesos(cliente.deuda)}</div>
          <div className="text-xs text-tenue">
            {cliente.limiteCredito === null
              ? "Sin límite de crédito"
              : `Límite ${pesos(cliente.limiteCredito)} · disponible ${pesos(Math.max(0, cliente.disponible ?? 0))}`}
          </div>
        </div>
      </header>

      <div className="mt-4 flex flex-wrap gap-2">
        <button
          type="button"
          onClick={() => setPagando(true)}
          disabled={cliente.deuda === 0}
          className="h-10 rounded-lg bg-acento px-4 font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
        >
          Registrar pago <span className="text-xs font-normal opacity-70">Ctrl+P</span>
        </button>
        <button
          type="button"
          onClick={() => imprimir("estado de cuenta", () => api.imprimirEstadoCuenta(clienteId))}
          className="flex h-10 items-center gap-2 rounded-lg border border-linea px-3 hover:bg-fondo"
        >
          <Printer size={16} aria-hidden /> Estado de cuenta
        </button>
        <button type="button" onClick={() => void pdf()} className="flex h-10 items-center gap-2 rounded-lg border border-linea px-3 hover:bg-fondo">
          <FileDown size={16} aria-hidden /> PDF
        </button>
        <span className="flex-1" />
        <button type="button" onClick={onEditar} className="flex h-10 items-center gap-2 rounded-lg px-3 text-tenue hover:bg-fondo hover:text-texto">
          <Pencil size={16} aria-hidden /> Editar
        </button>
        <button
          type="button"
          onClick={() => void cambiarActivo()}
          disabled={cliente.activo && cliente.deuda > 0}
          title={cliente.activo && cliente.deuda > 0 ? "Solo se desactiva un cliente que no debe nada" : undefined}
          className="h-10 rounded-lg px-3 text-tenue hover:bg-fondo hover:text-texto disabled:opacity-50 disabled:hover:bg-transparent"
        >
          {cliente.activo ? "Desactivar" : "Activar"}
        </button>
      </div>

      {hecho && (
        <p role="status" className="mt-3 rounded-lg bg-exito-fondo px-3 py-2 font-medium text-exito tabular-nums">
          {hecho}
        </p>
      )}
      {aviso && (
        <p
          role="status"
          className={`mt-3 flex items-center gap-3 rounded-lg px-3 py-2 ${
            aviso.tipo === "error" ? "bg-peligro/10 text-peligro" : aviso.tipo === "ok" ? "bg-exito-fondo text-exito" : "bg-fondo text-tenue"
          }`}
        >
          <span className="min-w-0 flex-1 break-all">{aviso.texto}</span>
          {aviso.reintentar && (
            <button type="button" onClick={aviso.reintentar} className="font-medium underline">
              Reintentar
            </button>
          )}
        </p>
      )}

      <section className="mt-5">
        <div className="flex items-baseline justify-between">
          <h3 className="font-semibold">Ventas pendientes</h3>
          {elegidas.length > 0 && (
            <span className="text-tenue tabular-nums">
              {elegidas.length} marcada{elegidas.length === 1 ? "" : "s"} · {pesos(elegidas.reduce((s, v) => s + v.saldo, 0))}
            </span>
          )}
        </div>
        {pendientes.length === 0 ? (
          <p className="mt-2 rounded-lg border border-linea p-4 text-center text-tenue">No debe nada.</p>
        ) : (
          <table className="mt-2 w-full tabular-nums">
            <thead className="text-left text-xs text-tenue">
              <tr className="border-b border-linea">
                <th className="w-8 py-2" aria-label="Marcar" />
                <th className="py-2">Venta</th>
                <th className="py-2">Fecha</th>
                <th className="py-2 text-right">Total</th>
                <th className="py-2 text-right">Pagado</th>
                <th className="py-2 text-right">Debe</th>
              </tr>
            </thead>
            <tbody>
              {pendientes.map((v) => (
                <FilaPendiente
                  key={v.ventaId}
                  venta={v}
                  marcada={marcadas.has(v.ventaId)}
                  alerta={diasDesde(v.fechaHora) >= diasAlerta}
                  onMarcar={() => alternar(v.ventaId)}
                  onIrAVenta={onIrAVenta}
                />
              ))}
            </tbody>
          </table>
        )}
        {pendientes.length > 1 && (
          <p className="mt-1 text-xs text-tenue">Marca ventas para que el pago vaya primero a ellas; si no, se paga de la más antigua.</p>
        )}
      </section>

      <section className="mt-6">
        <h3 className="font-semibold">Pagos</h3>
        {pagos.length === 0 ? (
          <p className="mt-2 text-tenue">Todavía no hay pagos.</p>
        ) : (
          <ul className="mt-2 divide-y divide-linea rounded-lg border border-linea">
            {pagos.map((p) => (
              <li key={p.id} className={`flex flex-wrap items-center gap-x-4 gap-y-1 px-3 py-2 ${p.anuladoEn ? "text-tenue" : ""}`}>
                <span className="tabular-nums">{fechaHora(p.fecha)}</span>
                <span>{NOMBRE_MEDIO[p.medioPago]}</span>
                <span className={`font-semibold tabular-nums ${p.anuladoEn ? "line-through" : ""}`}>{pesos(p.monto)}</span>
                <span className="min-w-0 flex-1 truncate text-xs text-tenue">
                  {p.anuladoEn
                    ? `Anulado: ${p.motivoAnulacion ?? ""}`
                    : p.abonos.map((a) => `${folio(a.folio)} ${pesos(a.monto)}`).join(" · ")}
                </span>
                <button
                  type="button"
                  onClick={() => imprimir("comprobante de pago", () => api.imprimirPago(p.id))}
                  className="h-8 rounded-lg px-2 text-tenue hover:bg-fondo hover:text-texto"
                >
                  Comprobante
                </button>
                {!p.anuladoEn && (
                  <button
                    type="button"
                    onClick={() => setAnulando(p)}
                    className="h-8 rounded-lg px-2 text-peligro hover:bg-peligro/10"
                  >
                    Anular
                  </button>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>

      {pagadas.length > 0 && (
        <details className="mt-6">
          <summary className="cursor-pointer font-semibold">Ventas ya pagadas ({pagadas.length})</summary>
          <ul className="mt-2 divide-y divide-linea rounded-lg border border-linea tabular-nums">
            {pagadas.map((v) => (
              <li key={v.ventaId} className="flex items-center gap-4 px-3 py-2">
                <button type="button" onClick={() => onIrAVenta(v.folio)} className="font-medium hover:underline">
                  {folio(v.folio)}
                </button>
                <span className="text-tenue">{fechaHora(v.fechaHora)}</span>
                <span className="flex-1" />
                <span>{pesos(v.total)}</span>
                <span className="text-xs text-tenue">{v.pagadaEn ? `pagada el ${fechaHora(v.pagadaEn)}` : "pagada"}</span>
              </li>
            ))}
          </ul>
        </details>
      )}

      {pagando && (
        <RegistrarPago
          cliente={cliente}
          marcadas={elegidas}
          redondeoActivo={config?.redondeoEfectivo ?? true}
          onCerrar={() => setPagando(false)}
          onRegistrado={(r) => {
            setPagando(false);
            setMarcadas(new Set());
            void leer();
            onCambio();
            setHecho(
              `Pago de ${pesos(r.pago.monto)} registrado` +
                (r.pago.vuelto ? ` · vuelto ${pesos(r.pago.vuelto)}` : "") +
                ` · queda debiendo ${pesos(r.deudaRestante)}`,
            );
            setAviso(null);
            if (config?.impresora) imprimir("comprobante de pago", () => api.imprimirPago(r.pago.id));
          }}
        />
      )}
      {anulando && (
        <DialogoAnularPago
          pago={anulando}
          onCerrar={() => setAnulando(null)}
          onAnular={async (motivo) => {
            const r = await conPin((pin) => api.anularPago(anulando.id, motivo, pin), {
              accion: `Anular el pago de ${pesos(anulando.monto)} del ${fechaHora(anulando.fecha)}`,
            });
            setAnulando(null);
            void leer();
            onCambio();
            setAviso(null);
            setHecho(
              `Pago anulado${r.pago.medioPago === "efectivo" ? `: devuelve ${pesos(r.pago.monto + r.pago.redondeo)} de la caja` : ""} · debe ${pesos(r.deudaRestante)}`,
            );
          }}
          esCancelado={esCancelado}
        />
      )}
    </div>
  );
}

function FilaPendiente({
  venta: v,
  marcada,
  alerta,
  onMarcar,
  onIrAVenta,
}: {
  venta: VentaFiada;
  marcada: boolean;
  alerta: boolean;
  onMarcar: () => void;
  onIrAVenta: (folio: number) => void;
}) {
  const dias = diasDesde(v.fechaHora);
  return (
    <tr className={`border-b border-linea ${marcada ? "bg-fondo" : ""}`}>
      <td className="py-2">
        <input type="checkbox" checked={marcada} onChange={onMarcar} aria-label={`Marcar ${folio(v.folio)}`} />
      </td>
      <td className="py-2">
        <button type="button" onClick={() => onIrAVenta(v.folio)} className="font-medium hover:underline">
          {folio(v.folio)}
        </button>
      </td>
      <td className="py-2">
        {fechaHora(v.fechaHora)}
        <span className={`ml-2 text-xs ${alerta ? "rounded bg-aviso-fondo px-1.5 py-0.5 font-medium text-aviso" : "text-tenue"}`}>
          {dias === 0 ? "hoy" : dias === 1 ? "hace 1 día" : `hace ${dias} días`}
        </span>
      </td>
      <td className="py-2 text-right">{pesos(v.total)}</td>
      <td className="py-2 text-right text-tenue">{v.total - v.saldo > 0 ? pesos(v.total - v.saldo) : "—"}</td>
      <td className="py-2 text-right font-semibold">{pesos(v.saldo)}</td>
    </tr>
  );
}

function DialogoAnularPago({
  pago,
  onCerrar,
  onAnular,
  esCancelado,
}: {
  pago: Pago;
  onCerrar: () => void;
  onAnular: (motivo: string) => Promise<void>;
  esCancelado: (e: unknown) => boolean;
}) {
  const [motivo, setMotivo] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);
  const efectivo = pago.medioPago === "efectivo";

  const anular = async () => {
    if (enviando || !motivo.trim()) return;
    setEnviando(true);
    setError(null);
    try {
      await onAnular(motivo);
    } catch (e) {
      if (!esCancelado(e)) setError(textoError(e));
      setEnviando(false);
    }
  };

  return (
    <Dialogo titulo="Anular pago" onCerrar={onCerrar} ancho="w-[28rem]">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void anular();
        }}
        className="mt-3 space-y-4"
      >
        <p className="text-tenue">
          Pago de <span className="font-semibold text-texto">{pesos(pago.monto)}</span> en {NOMBRE_MEDIO[pago.medioPago].toLowerCase()} del{" "}
          {fechaHora(pago.fecha)}. Las ventas que pagó vuelven a deberse. El pago queda guardado como anulado: no se borra.
          {efectivo && ` Los ${pesos(pago.monto + pago.redondeo)} salen de la caja abierta.`}
        </p>
        <label className="block">
          <span className="text-tenue">Motivo (obligatorio)</span>
          <input
            autoFocus
            value={motivo}
            onChange={(e) => setMotivo(e.target.value)}
            maxLength={200}
            placeholder="Se ingresó mal el monto"
            className="mt-1 h-10 w-full rounded-lg border border-linea bg-fondo px-3 outline-none focus:border-texto"
          />
        </label>
        {error && <p className="rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}
        <div className="flex gap-2">
          <button type="button" onClick={onCerrar} className="h-11 flex-1 rounded-lg text-tenue hover:bg-fondo hover:text-texto">
            Volver
          </button>
          <button
            type="submit"
            disabled={enviando || !motivo.trim()}
            className="h-11 flex-1 rounded-lg bg-peligro font-semibold text-superficie disabled:bg-linea disabled:text-tenue"
          >
            Anular pago
          </button>
        </div>
      </form>
    </Dialogo>
  );
}
