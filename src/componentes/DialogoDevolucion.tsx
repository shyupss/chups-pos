import { Scale } from "lucide-react";
import { useState } from "react";
import { api, textoError, type DetalleVenta, type DevolucionRegistrada, type LineaDetalle, type MedioPago } from "../api";
import { cantidad, folio, NOMBRE_MEDIO, pesos, pesosConSigno, redondeoEfectivo } from "../formato";
import { MEDIOS_REEMBOLSO } from "./DialogoAnular";
import Dialogo from "./Dialogo";
import DialogoPeso from "./DialogoPeso";
import Opciones from "./Opciones";
import { esCancelado, useConPin } from "./PedirPin";

interface Props {
  venta: DetalleVenta;
  /** Si el efectivo se redondea a la decena (para el monto estimado). */
  redondeoActivo: boolean;
  onDevuelta: (r: DevolucionRegistrada) => void;
  onCerrar: () => void;
}

/** Lo que se va a devolver de cada línea (milésimas) y si vuelve al stock. */
interface Eleccion {
  mil: number;
  vuelve: boolean;
}

// Misma regla que Rust: subtotal × cantidad / vendido, redondeado al peso, sobre lo acumulado
// (así varias devoluciones suman exacto el subtotal). Solo para mostrar: el monto lo calcula Rust.
const proporcion = (parte: number, num: number, den: number) =>
  den === 0 ? 0 : Number((BigInt(parte) * BigInt(num) * 2n + BigInt(den)) / (BigInt(den) * 2n));
const montoLinea = (l: LineaDetalle, mil: number) =>
  proporcion(l.subtotal, l.devueltoMil + mil, l.cantidadMil) - proporcion(l.subtotal, l.devueltoMil, l.cantidadMil);

/** Devolver parte de la venta: cuánto de cada línea, si vuelve al stock, cómo se reembolsa y por qué. */
export default function DialogoDevolucion({ venta, redondeoActivo, onDevuelta, onCerrar }: Props) {
  const conPin = useConPin();
  const fiada = venta.medioPago === "fiado";
  const [elegido, setElegido] = useState<Record<number, Eleccion>>({});
  const [medio, setMedio] = useState<Exclude<MedioPago, "fiado">>(
    venta.medioPago === "fiado" ? "efectivo" : venta.medioPago,
  );
  const [motivo, setMotivo] = useState("");
  const [pesando, setPesando] = useState<LineaDetalle | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);

  const de = (l: LineaDetalle): Eleccion => elegido[l.id] ?? { mil: 0, vuelve: true };
  const fijar = (l: LineaDetalle, cambio: Partial<Eleccion>) =>
    setElegido((e) => ({ ...e, [l.id]: { ...de(l), ...cambio } }));
  const queda = (l: LineaDetalle) => l.cantidadMil - l.devueltoMil;

  const items = venta.lineas.filter((l) => de(l).mil > 0);
  const subtotal = items.reduce((s, l) => s + montoLinea(l, de(l).mil), 0);
  const redondeo = (!fiada || venta.abonado > 0) && medio === "efectivo" && redondeoActivo ? redondeoEfectivo(subtotal) : 0;
  const listo = items.length > 0 && motivo.trim() !== "";

  const devolver = async () => {
    if (!listo || enviando) return;
    setEnviando(true);
    setError(null);
    try {
      const r = await conPin(
        (pin) =>
          api.registrarDevolucion(
            venta.id,
            items.map((l) => ({ ventaItemId: l.id, cantidadMil: de(l).mil, vuelveAStock: de(l).vuelve })),
            fiada && venta.abonado === 0 ? null : medio,
            motivo,
            pin,
          ),
        { accion: `Devolver productos de la venta ${folio(venta.folio)}` },
      );
      onDevuelta(r);
    } catch (e) {
      if (!esCancelado(e)) setError(textoError(e));
      setEnviando(false);
    }
  };

  return (
    <Dialogo titulo={`Devolver productos · venta ${folio(venta.folio)}`} onCerrar={onCerrar} ancho="w-[50rem]">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void devolver();
        }}
        className="mt-4"
      >
        <table className="w-full">
          <thead className="text-left text-xs uppercase text-tenue">
            <tr className="h-8">
              <th className="font-medium">Producto</th>
              <th className="px-2 text-right font-medium">Vendido</th>
              <th className="px-2 text-right font-medium">Ya devuelto</th>
              <th className="px-2 text-center font-medium">Devolver</th>
              <th className="px-2 text-center font-medium">Vuelve al stock</th>
              <th className="text-right font-medium">Monto</th>
            </tr>
          </thead>
          <tbody>
            {venta.lineas.map((l) => {
              const e = de(l);
              const resto = queda(l);
              return (
                <tr key={l.id} className={`h-12 border-t border-linea ${resto === 0 ? "text-tenue" : ""}`}>
                  <td className="py-1">
                    <div className="max-w-56 truncate" title={l.nombre}>
                      {l.nombre}
                    </div>
                    <div className="text-xs text-tenue">
                      {pesos(l.precioUnitario)}
                      {l.unidad === "kg" ? "/kg" : " c/u"} · subtotal {pesos(l.subtotal)}
                    </div>
                  </td>
                  <td className="px-2 text-right">{cantidad(l.cantidadMil, l.unidad)}</td>
                  <td className="px-2 text-right">{l.devueltoMil > 0 ? cantidad(l.devueltoMil, l.unidad) : "—"}</td>
                  <td className="px-2">
                    {resto === 0 ? (
                      <span className="block text-center text-xs">Devuelto completo</span>
                    ) : l.unidad === "kg" ? (
                      <div className="flex items-center justify-center gap-2">
                        <button
                          type="button"
                          onClick={() => setPesando(l)}
                          className="flex h-8 items-center gap-1.5 rounded-lg border border-linea px-2 hover:bg-fondo"
                        >
                          <Scale size={14} aria-hidden />
                          {e.mil > 0 ? cantidad(e.mil, "kg") : "Pesar…"}
                        </button>
                        {e.mil > 0 && (
                          <button
                            type="button"
                            onClick={() => fijar(l, { mil: 0 })}
                            aria-label={`No devolver ${l.nombre}`}
                            className="h-8 rounded-lg px-2 text-tenue hover:bg-fondo hover:text-texto"
                          >
                            ×
                          </button>
                        )}
                      </div>
                    ) : (
                      <div className="flex items-center justify-center gap-1">
                        <BotonChico onClick={() => fijar(l, { mil: Math.max(0, e.mil - 1000) })}>−</BotonChico>
                        <input
                          value={e.mil / 1000}
                          onChange={(ev) => {
                            const n = Number(ev.target.value.replace(/\D/g, "")) || 0;
                            fijar(l, { mil: Math.min(resto, n * 1000) });
                          }}
                          onFocus={(ev) => ev.currentTarget.select()}
                          inputMode="numeric"
                          aria-label={`Unidades de ${l.nombre} a devolver`}
                          className="h-8 w-12 rounded-lg border border-borde-control bg-superficie px-1 text-center tabular-nums focus:border-acento"
                        />
                        <BotonChico onClick={() => fijar(l, { mil: Math.min(resto, e.mil + 1000) })}>+</BotonChico>
                      </div>
                    )}
                  </td>
                  <td className="px-2 text-center">
                    <input
                      type="checkbox"
                      checked={e.vuelve}
                      disabled={resto === 0}
                      onChange={(ev) => fijar(l, { vuelve: ev.target.checked })}
                      aria-label={`${l.nombre} vuelve al stock`}
                      title="Desmárcalo si viene dañado o vencido: queda como merma"
                      className="size-4"
                    />
                  </td>
                  <td className="text-right tabular-nums">{e.mil > 0 ? pesos(montoLinea(l, e.mil)) : "—"}</td>
                </tr>
              );
            })}
          </tbody>
        </table>

        <div className="mt-4 grid grid-cols-2 gap-6 border-t border-linea pt-4">
          <div className="space-y-4">
            {fiada ? (
              <>
                <p className="rounded-lg bg-acento/12 px-3 py-2 text-acento">
                  Venta fiada{venta.cliente ? ` de ${venta.cliente}` : ""}: baja la deuda (debe {pesos(venta.saldo ?? 0)} de
                  esta venta).
                  {venta.abonado > 0 && " Si devuelve más de lo que debe, lo que sobra pasa a sus otras deudas o se le devuelve."}
                </p>
                {venta.abonado > 0 && <div>
            <span className="text-tenue">Si sobra, devolver en</span>
            <div className="mt-1">
              <Opciones nombre="medio-devolucion" etiqueta="Medio de reembolso" opciones={MEDIOS_REEMBOLSO} valor={medio} onCambiar={setMedio} />
            </div>
          </div>}
              </>
            ) : (
              <div>
                <span className="text-tenue">Devolver el dinero en</span>
                <div className="mt-1">
                  <Opciones nombre="medio-devolucion" etiqueta="Medio de reembolso" opciones={MEDIOS_REEMBOLSO} valor={medio} onCambiar={setMedio} />
                </div>
                <p className="mt-1 text-xs text-tenue">Se pagó con {NOMBRE_MEDIO[venta.medioPago].toLowerCase()}.</p>
              </div>
            )}
            <label className="block">
              <span className="text-tenue">Motivo (obligatorio)</span>
              <input
                value={motivo}
                onChange={(e) => setMotivo(e.target.value)}
                maxLength={200}
                placeholder="Por ejemplo, venía vencido"
                className="mt-1 h-10 w-full rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento"
              />
            </label>
          </div>
          <dl className="space-y-1 self-end tabular-nums">
            <div className="flex justify-between text-tenue">
              <dt>Productos</dt>
              <dd>{pesos(subtotal)}</dd>
            </div>
            {redondeo !== 0 && (
              <div className="flex justify-between text-tenue">
                <dt>Redondeo del efectivo</dt>
                <dd>{pesosConSigno(redondeo)}</dd>
              </div>
            )}
            <div className="flex items-baseline justify-between">
              <dt>{fiada ? "Baja la deuda" : "A devolver"} (estimado)</dt>
              <dd className="text-3xl font-semibold">{pesos(subtotal + redondeo)}</dd>
            </div>
          </dl>
        </div>

        {error && <p className="mt-4 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}
        <div className="mt-5 flex gap-2">
          <button type="button" onClick={onCerrar} className="h-11 flex-1 rounded-lg text-tenue hover:bg-fondo hover:text-texto">
            Volver <span className="text-xs opacity-60">Esc</span>
          </button>
          <button
            type="submit"
            disabled={!listo || enviando}
            className="h-11 flex-[2] rounded-lg bg-acento font-semibold text-sobre-acento disabled:bg-linea disabled:text-tenue"
          >
            Devolver {items.length > 0 ? pesos(subtotal + redondeo) : ""}{" "}
            <span className="text-xs font-normal opacity-70">Enter</span>
          </button>
        </div>
      </form>

      {pesando && (
        <DialogoPeso
          producto={{ nombre: pesando.nombre, precioVenta: pesando.precioUnitario }}
          inicialMil={de(pesando).mil || undefined}
          maximoMil={queda(pesando)}
          accion="Devolver"
          onConfirmar={(mil) => {
            fijar(pesando, { mil });
            setPesando(null);
          }}
          onCancelar={() => setPesando(null)}
        />
      )}
    </Dialogo>
  );
}

function BotonChico({ onClick, children }: { onClick: () => void; children: string }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="size-8 rounded-lg border border-linea text-tenue hover:bg-fondo hover:text-texto"
    >
      {children}
    </button>
  );
}
