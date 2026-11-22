import Boton from "./Boton";
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { api, textoError, type Conteo, type ResumenTurno } from "../api";
import { aEntero, fechaHora, NOMBRE_MEDIO, pesos, pesosConSigno, soloDigitos } from "../formato";
import Dialogo from "./Dialogo";
import Opciones, { type Opcion } from "./Opciones";
import { esCancelado, useConPin } from "./PedirPin";

const BILLETES = [20000, 10000, 5000, 2000, 1000];
const MONEDAS = [500, 100, 50, 10];

type Modo = "denominaciones" | "total";
const MODOS: Opcion<Modo>[] = [
  { id: "denominaciones", nombre: "Por billete y moneda" },
  { id: "total", nombre: "Monto total" },
];

type Tarea = { estado: "trabajando" } | { estado: "ok"; texto: string } | { estado: "error"; texto: string };

interface Props {
  turnoId: number;
  /** Hay impresora elegida: el cierre se imprime solo al confirmar. */
  imprimir: boolean;
  onImpresion: () => void;
  onCancelar: () => void;
  /** La caja quedó cerrada (se llama al salir del último paso). */
  onCerrada: () => void;
}

/** Cerrar caja en dos pasos: contar el efectivo y revisar el resumen contra lo esperado. */
export default function CerrarCaja({ turnoId, imprimir, onImpresion, onCancelar, onCerrada }: Props) {
  const conPin = useConPin();
  const [paso, setPaso] = useState<"conteo" | "resumen" | "listo">("conteo");
  const [modo, setModo] = useState<Modo>("denominaciones");
  const [cantidades, setCantidades] = useState<Record<number, string>>({});
  const [total, setTotal] = useState("");
  const [nota, setNota] = useState("");
  const [resumen, setResumen] = useState<ResumenTurno | null>(null);
  const [cerrado, setCerrado] = useState<ResumenTurno | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);
  const [impresion, setImpresion] = useState<Tarea | null>(null);
  const [pdf, setPdf] = useState<Tarea | null>(null);

  const detalle: Conteo[] = [...BILLETES, ...MONEDAS]
    .map((d) => ({ denominacion: d, cantidad: Number(cantidades[d] || 0) }))
    .filter((c) => c.cantidad > 0);
  const contado = modo === "total" ? (aEntero(total) ?? 0) : detalle.reduce((s, c) => s + c.denominacion * c.cantidad, 0);

  // El resumen se lee al entrar y otra vez al pasar al segundo paso, por si se vendió entremedio.
  const leerResumen = useCallback(
    () =>
      api
        .resumenTurno(turnoId)
        .then(setResumen)
        .catch((e) => setError(textoError(e))),
    [turnoId],
  );
  useEffect(() => {
    void leerResumen();
  }, [leerResumen]);

  const imprimirCierre = async () => {
    setImpresion({ estado: "trabajando" });
    try {
      await api.imprimirCierre(turnoId);
      setImpresion({ estado: "ok", texto: "Cierre impreso" });
    } catch (e) {
      setImpresion({ estado: "error", texto: textoError(e) });
    } finally {
      onImpresion();
    }
  };

  const guardarPdf = async (abrir: boolean) => {
    setPdf({ estado: "trabajando" });
    try {
      const ruta = await api.guardarCierrePdf(turnoId, abrir);
      setPdf({ estado: "ok", texto: `PDF guardado en ${ruta}` });
    } catch (e) {
      setPdf({ estado: "error", texto: textoError(e) });
    }
  };

  const confirmar = async () => {
    if (enviando) return;
    setEnviando(true);
    setError(null);
    try {
      const r = await conPin(
        (pin) => api.cerrarTurno(contado, modo === "denominaciones" ? detalle : null, nota.trim() || null, pin),
        { accion: "Cerrar la caja" },
      );
      setCerrado(r);
      setPaso("listo");
      if (imprimir) void imprimirCierre();
    } catch (e) {
      if (!esCancelado(e)) setError(textoError(e));
    } finally {
      setEnviando(false);
    }
  };

  const esperado = resumen?.efectivo.esperado ?? 0;
  const titulo = paso === "conteo" ? "Cerrar caja · 1 de 2: contar el efectivo" : paso === "resumen" ? "Cerrar caja · 2 de 2: resumen" : "Caja cerrada";

  return (
    <Dialogo titulo={titulo} onCerrar={paso === "listo" ? onCerrada : onCancelar} ancho="w-[44rem]">
      {paso === "conteo" && (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            setError(null);
            void leerResumen();
            setPaso("resumen");
          }}
          className="mt-4"
        >
          <Opciones nombre="modo-conteo" etiqueta="Cómo contar" opciones={MODOS} valor={modo} onCambiar={setModo} />
          {modo === "denominaciones" ? (
            <div className="mt-4 grid grid-cols-2 gap-6">
              <Denominaciones titulo="Billetes" lista={BILLETES} cantidades={cantidades} onCambiar={setCantidades} primero />
              <Denominaciones titulo="Monedas" lista={MONEDAS} cantidades={cantidades} onCambiar={setCantidades} />
            </div>
          ) : (
            <label className="mt-4 block">
              <span className="text-tenue">Efectivo contado</span>
              <input
                autoFocus
                value={total ? pesos(aEntero(total) ?? 0) : ""}
                onChange={(e) => setTotal(e.target.value)}
                placeholder="$0"
                inputMode="numeric"
                className="mt-1 h-13 w-full rounded-lg border border-borde-control bg-superficie px-4 text-2xl tabular-nums focus:border-acento"
              />
            </label>
          )}
          <div className="mt-5 flex items-baseline justify-between border-t border-linea pt-4">
            <span className="text-tenue">Efectivo contado</span>
            <span className="text-3xl font-semibold tabular-nums">{pesos(contado)}</span>
          </div>
          <Pie onVolver={onCancelar} volver="Cancelar" atajo="Esc" siguiente="Ver resumen" />
        </form>
      )}

      {paso === "resumen" && (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void confirmar();
          }}
          className="mt-4"
        >
          {!resumen ? (
            <p className="text-tenue">Cargando el resumen…</p>
          ) : (
            <div className="grid grid-cols-2 gap-6">
              <ResumenVentas r={resumen} />
              <div>
                <Titulo>Efectivo</Titulo>
                <dl className="space-y-1 tabular-nums">
                  <Par etiqueta="Monto inicial" valor={pesos(resumen.efectivo.montoInicial)} />
                  <Par etiqueta="Ventas en efectivo" valor={pesos(resumen.efectivo.ventas)} />
                  {resumen.efectivo.abonos !== 0 && <Par etiqueta="Abonos en efectivo" valor={pesos(resumen.efectivo.abonos)} />}
                  <Par etiqueta="Ingresos" valor={pesos(resumen.efectivo.ingresos)} />
                  <Par etiqueta="Retiros" valor={pesosConSigno(-resumen.efectivo.retiros)} />
                  <Par etiqueta="Reembolsos" valor={pesosConSigno(-resumen.efectivo.reembolsos)} />
                  <Par etiqueta="Esperado" valor={pesos(esperado)} fuerte />
                  <Par etiqueta="Contado" valor={pesos(contado)} fuerte />
                </dl>
                <Diferencia diferencia={contado - esperado} />
              </div>
            </div>
          )}
          <label className="mt-5 block">
            <span className="text-tenue">Nota (opcional)</span>
            <input
              value={nota}
              onChange={(e) => setNota(e.target.value)}
              maxLength={300}
              placeholder="Por ejemplo, por qué falta o sobra"
              className="mt-1 h-10 w-full rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento"
            />
          </label>
          {error && <p className="mt-4 rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">{error}</p>}
          <Pie
            onVolver={() => setPaso("conteo")}
            volver="Volver al conteo"
            siguiente={enviando ? "Cerrando…" : "Cerrar caja"}
            deshabilitado={!resumen || enviando}
          />
        </form>
      )}

      {paso === "listo" && cerrado && (
        <div className="mt-4">
          <p className="text-tenue">
            Turno abierto el {fechaHora(cerrado.turno.abiertoEn)} y cerrado el{" "}
            {cerrado.turno.cerradoEn && fechaHora(cerrado.turno.cerradoEn)}.
          </p>
          <dl className="mt-3 space-y-1 tabular-nums">
            <Par etiqueta="Ventas" valor={`${cerrado.cantidadVentas} · ${pesos(cerrado.totalVentas)}`} />
            <Par etiqueta="Esperado" valor={pesos(cerrado.turno.efectivoEsperado ?? 0)} />
            <Par etiqueta="Contado" valor={pesos(cerrado.turno.efectivoContado ?? 0)} fuerte />
          </dl>
          <Diferencia diferencia={cerrado.turno.diferencia ?? 0} />
          <div className="mt-4 space-y-2">
            {impresion && <Estado tarea={impresion} trabajando="Imprimiendo el cierre…" error="No se imprimió el cierre" />}
            {pdf && <Estado tarea={pdf} trabajando="Guardando el PDF…" error="No se guardó el PDF" />}
          </div>
          <div className="mt-5 flex flex-wrap gap-2">
            <Boton onClick={() => void imprimirCierre()} disabled={impresion?.estado === "trabajando"}>
              {impresion ? "Reimprimir" : "Imprimir"}
            </Boton>
            <Boton onClick={() => void guardarPdf(false)} disabled={pdf?.estado === "trabajando"}>
              Guardar PDF
            </Boton>
            {pdf?.estado === "ok" && <Boton onClick={() => void guardarPdf(true)}>Abrir PDF</Boton>}
            <Boton variante="primario" autoFocus onClick={onCerrada} className="ml-auto px-6">
              Listo
            </Boton>
          </div>
        </div>
      )}
    </Dialogo>
  );
}

function Denominaciones({
  titulo,
  lista,
  cantidades,
  onCambiar,
  primero,
}: {
  titulo: string;
  lista: number[];
  cantidades: Record<number, string>;
  onCambiar: (f: (c: Record<number, string>) => Record<number, string>) => void;
  primero?: boolean;
}) {
  return (
    <fieldset>
      <legend className="mb-2 text-xs font-medium text-tenue">{titulo}</legend>
      <div className="space-y-1.5">
        {lista.map((d, i) => {
          const n = Number(cantidades[d] || 0);
          return (
            <label key={d} className="grid grid-cols-[5rem_5rem_1fr] items-center gap-3 tabular-nums">
              <span className="font-medium">{pesos(d)}</span>
              <input
                autoFocus={primero && i === 0}
                value={cantidades[d] ?? ""}
                onChange={(e) => onCambiar((c) => ({ ...c, [d]: soloDigitos(e.target.value).slice(0, 6) }))}
                onFocus={(e) => e.currentTarget.select()}
                inputMode="numeric"
                placeholder="0"
                aria-label={`Cantidad de ${titulo.toLowerCase()} de ${pesos(d)}`}
                className="h-9 rounded-lg border border-borde-control bg-superficie px-2 text-right focus:border-acento"
              />
              <span className={`text-right ${n > 0 ? "" : "text-tenue"}`}>{pesos(d * n)}</span>
            </label>
          );
        })}
      </div>
    </fieldset>
  );
}

function ResumenVentas({ r }: { r: ResumenTurno }) {
  return (
    <div>
      <Titulo>Ventas por medio de pago</Titulo>
      <dl className="space-y-1 tabular-nums">
        {r.ventas.map((t) => (
          <Par key={t.medio} etiqueta={`${NOMBRE_MEDIO[t.medio]} (${t.cantidad})`} valor={pesos(t.monto)} tenue={t.cantidad === 0} />
        ))}
        <Par etiqueta={`Total (${r.cantidadVentas})`} valor={pesos(r.totalVentas)} fuerte />
        {r.redondeoVentas !== 0 && <Par etiqueta="Redondeo del efectivo" valor={pesosConSigno(r.redondeoVentas)} tenue />}
      </dl>
      {(r.anulaciones.length > 0 || r.devoluciones.length > 0 || r.pagosAnulados.length > 0) && (
        <>
          <Titulo>Anulaciones y devoluciones</Titulo>
          <dl className="space-y-1 tabular-nums">
            {r.anulaciones.map((t) => (
              <Par key={`a-${t.medio}`} etiqueta={`Anuladas · ${reembolso(t.medio)} (${t.cantidad})`} valor={pesosConSigno(-t.monto)} />
            ))}
            {r.devoluciones.map((t) => (
              <Par key={`d-${t.medio}`} etiqueta={`Devoluciones · ${reembolso(t.medio)} (${t.cantidad})`} valor={pesosConSigno(-t.monto)} />
            ))}
            {r.pagosAnulados.map((t) => (
              <Par key={`p-${t.medio}`} etiqueta={`Pagos de fiados anulados · ${reembolso(t.medio)} (${t.cantidad})`} valor={pesosConSigno(-t.monto)} />
            ))}
          </dl>
        </>
      )}
      {r.movimientos.length > 0 && (
        <>
          <Titulo>Ingresos y retiros</Titulo>
          <dl className="space-y-1 tabular-nums">
            {r.movimientos.map((m) => (
              <Par
                key={m.id}
                etiqueta={`${m.tipo === "ingreso" ? "Ingreso" : "Retiro"}: ${m.motivo}`}
                valor={pesosConSigno(m.tipo === "ingreso" ? m.monto : -m.monto)}
              />
            ))}
          </dl>
        </>
      )}
    </div>
  );
}

const reembolso = (medio: keyof typeof NOMBRE_MEDIO) => (medio === "fiado" ? "fiado, sin reembolso" : NOMBRE_MEDIO[medio].toLowerCase());

function Titulo({ children }: { children: ReactNode }) {
  return <h3 className="mt-3 mb-2 text-xs font-medium text-tenue first:mt-0">{children}</h3>;
}

function Par({ etiqueta, valor, fuerte, tenue }: { etiqueta: string; valor: string; fuerte?: boolean; tenue?: boolean }) {
  return (
    <div className={`flex items-baseline justify-between gap-3 ${fuerte ? "font-semibold" : ""} ${tenue ? "text-tenue" : ""}`}>
      <dt className="min-w-0 truncate" title={etiqueta}>
        {etiqueta}
      </dt>
      <dd className="shrink-0">{valor}</dd>
    </div>
  );
}

/** Verde si cuadra, aviso si sobra y rojo si falta. */
function Diferencia({ diferencia }: { diferencia: number }) {
  const [clase, texto] =
    diferencia === 0
      ? ["bg-exito-fondo text-exito", "Caja cuadrada"]
      : diferencia > 0
        ? ["bg-aviso-fondo text-aviso", `Sobran ${pesos(diferencia)}`]
        : ["bg-peligro-fondo text-peligro", `Faltan ${pesos(-diferencia)}`];
  return (
    <div role="status" className={`mt-3 flex items-baseline justify-between rounded-lg px-4 py-3 tabular-nums ${clase}`}>
      <span className="font-semibold">{texto}</span>
      <span className="text-2xl font-semibold">{pesosConSigno(diferencia)}</span>
    </div>
  );
}

function Estado({ tarea, trabajando, error }: { tarea: Tarea; trabajando: string; error: string }) {
  if (tarea.estado === "error") {
    return (
      <p role="alert" className="rounded-lg bg-peligro-fondo px-3 py-2 text-peligro">
        {error}: {tarea.texto}
      </p>
    );
  }
  return (
    <p role="status" className="break-all text-tenue">
      {tarea.estado === "trabajando" ? trabajando : tarea.texto}
    </p>
  );
}

function Pie({
  onVolver,
  volver,
  atajo,
  siguiente,
  deshabilitado,
}: {
  onVolver: () => void;
  volver: string;
  atajo?: string;
  siguiente: string;
  deshabilitado?: boolean;
}) {
  return (
    <div className="mt-6 flex gap-2">
      <Boton variante="terciario" atajo={atajo} onClick={onVolver} className="flex-1">
        {volver}
      </Boton>
      <Boton type="submit" variante="primario" atajo="Enter" disabled={deshabilitado} className="flex-[2]">
        {siguiente}
      </Boton>
    </div>
  );
}
