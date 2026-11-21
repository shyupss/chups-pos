import { UserPlus } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api, textoError, type Cliente, type MedioPago, type NuevaVenta, type VentaConfirmada } from "../api";
import { aEntero, pesos, redondeoEfectivo } from "../formato";
import { esCancelado, useConPin } from "./PedirPin";

interface Props {
  items: NuevaVenta["items"];
  totalEstimado: number;
  /** Si el efectivo se redondea a la decena (config redondeo_efectivo). */
  redondeoActivo: boolean;
  onCancelar: () => void;
  onConfirmada: (venta: VentaConfirmada) => void;
}

const MEDIOS: { id: MedioPago; nombre: string; tecla: string }[] = [
  { id: "efectivo", nombre: "Efectivo", tecla: "1" },
  { id: "debito", nombre: "Débito", tecla: "2" },
  { id: "credito", nombre: "Crédito", tecla: "3" },
  { id: "transferencia", nombre: "Transferencia", tecla: "4" },
  { id: "fiado", nombre: "Fiado", tecla: "5" },
];

const BILLETES = [1000, 2000, 5000, 10000, 20000];

export default function Cobro({ items, totalEstimado, redondeoActivo, onCancelar, onConfirmada }: Props) {
  const conPin = useConPin();
  const [medio, setMedio] = useState<MedioPago>("efectivo");
  const [recibido, setRecibido] = useState("");
  const [cliente, setCliente] = useState<Cliente | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [enviando, setEnviando] = useState(false);
  const campoRecibido = useRef<HTMLInputElement>(null);

  // Solo para mostrar: el redondeo que vale lo aplica Rust al confirmar, con la misma regla.
  const redondeo = medio === "efectivo" && redondeoActivo ? redondeoEfectivo(totalEstimado) : 0;
  const aPagar = totalEstimado + redondeo;
  const montoRecibido = medio === "efectivo" ? aEntero(recibido) : null;
  const vuelto = montoRecibido !== null ? montoRecibido - aPagar : 0;
  const falta = medio === "efectivo" && montoRecibido !== null && montoRecibido < aPagar;
  const fiado = medio === "fiado";
  // Estimación para avisar antes de confirmar; el límite lo hace cumplir Rust.
  const sobreLimite =
    fiado && cliente !== null && cliente.limiteCredito !== null && cliente.deuda + aPagar > cliente.limiteCredito;
  const bloqueado = enviando || falta || (fiado && cliente === null);

  useEffect(() => {
    if (medio === "efectivo") campoRecibido.current?.focus();
  }, [medio]);

  const confirmar = async () => {
    if (bloqueado) return;
    setEnviando(true);
    setError(null);
    const venta: NuevaVenta = {
      items,
      medioPago: medio,
      // Vacío en efectivo = pagó justo.
      montoRecibido: medio === "efectivo" ? (montoRecibido ?? aPagar) : null,
      descuento: 0,
      clienteId: fiado ? (cliente?.id ?? null) : null,
    };
    try {
      const v = sobreLimite
        ? await conPin((pin) => api.confirmarVenta(venta, true, pin), {
            accion: `Fiar a ${cliente?.nombre} por sobre su límite de crédito`,
          })
        : await api.confirmarVenta(venta);
      onConfirmada(v);
    } catch (e) {
      if (!esCancelado(e)) setError(textoError(e));
      setEnviando(false);
    }
  };

  useEffect(() => {
    const alPresionar = (e: KeyboardEvent) => {
      // Mientras el PIN u otro diálogo está abierto, sus teclas no son del cobro.
      if (document.querySelector('[role="dialog"]')) return;
      const enCliente = e.target instanceof HTMLElement && e.target.dataset.cliente !== undefined;
      if (e.key === "Escape") return onCancelar();
      if ((e.key === "Enter" && !enCliente) || e.key === "F12") {
        e.preventDefault();
        return void confirmar();
      }
      const m = MEDIOS.find((x) => x.tecla === e.key);
      if (m && !(e.target instanceof HTMLInputElement)) setMedio(m.id);
      if (m && e.target instanceof HTMLInputElement && e.altKey) {
        e.preventDefault();
        setMedio(m.id);
      }
    };
    window.addEventListener("keydown", alPresionar);
    return () => window.removeEventListener("keydown", alPresionar);
  });

  return (
    <div className="fixed inset-0 z-20 flex items-center justify-center bg-velo">
      <div className="w-[34rem] rounded-lg border border-linea bg-superficie p-6 shadow-xl">
        <div className="flex items-baseline justify-between">
          <h2 className="text-lg font-semibold">Cobrar</h2>
          <span className="text-3xl font-semibold tabular-nums">{pesos(aPagar)}</span>
        </div>
        {redondeo !== 0 && (
          <dl className="mt-1 space-y-0.5 text-tenue tabular-nums">
            <div className="flex justify-between">
              <dt>Total productos</dt>
              <dd>{pesos(totalEstimado)}</dd>
            </div>
            <div className="flex justify-between">
              <dt>Redondeo</dt>
              <dd>
                {redondeo > 0 ? "+" : "−"}
                {pesos(Math.abs(redondeo))}
              </dd>
            </div>
          </dl>
        )}

        <div className="mt-4 grid grid-cols-5 gap-2">
          {MEDIOS.map((m) => (
            <button
              key={m.id}
              onClick={() => setMedio(m.id)}
              className={`rounded-lg border px-1 py-3 ${
                medio === m.id ? "border-texto bg-texto font-medium text-superficie" : "border-linea hover:bg-fondo"
              }`}
            >
              {m.nombre}
              <div className="text-xs opacity-60">{medio === "efectivo" || fiado ? `Alt+${m.tecla}` : m.tecla}</div>
            </button>
          ))}
        </div>

        {medio === "efectivo" && (
          <div className="mt-5">
            <label className="text-tenue">Recibido (vacío = justo)</label>
            <input
              ref={campoRecibido}
              value={recibido ? pesos(aEntero(recibido) ?? 0) : ""}
              onChange={(e) => setRecibido(e.target.value)}
              className="mt-1 h-13 w-full rounded-lg border border-linea bg-fondo px-4 text-2xl tabular-nums outline-none focus:border-texto"
              inputMode="numeric"
              placeholder={pesos(aPagar)}
            />
            <div className="mt-2 flex gap-2">
              {BILLETES.filter((b) => b >= aPagar || b === 20000).map((b) => (
                <button key={b} onClick={() => setRecibido(String(b))} className="h-8 rounded-lg border border-linea px-2 tabular-nums hover:bg-fondo">
                  {pesos(b)}
                </button>
              ))}
            </div>
            <div className="mt-4 flex items-baseline justify-between">
              <span className="text-tenue">Vuelto</span>
              <span className={`text-3xl font-semibold tabular-nums ${falta ? "text-peligro" : ""}`}>
                {falta ? `Faltan ${pesos(-vuelto)}` : pesos(Math.max(0, vuelto))}
              </span>
            </div>
          </div>
        )}

        {fiado && (
          <ElegirCliente
            cliente={cliente}
            onElegir={(c) => {
              setCliente(c);
              setError(null);
            }}
            aPagar={aPagar}
          />
        )}

        {sobreLimite && cliente && (
          <p className="mt-3 rounded-lg bg-aviso-fondo px-3 py-2 text-aviso">
            Supera el límite de crédito de {cliente.nombre} en {pesos(cliente.deuda + aPagar - (cliente.limiteCredito ?? 0))}.
            Se puede fiar igual con autorización.
          </p>
        )}
        {error && <p className="mt-4 rounded-lg bg-peligro/10 px-3 py-2 text-peligro">{error}</p>}

        <div className="mt-6 flex gap-2">
          <button onClick={onCancelar} className="h-12 flex-1 rounded-lg text-tenue hover:bg-fondo hover:text-texto">
            Volver <span className="text-xs opacity-60">Esc</span>
          </button>
          <button
            onClick={() => void confirmar()}
            disabled={bloqueado}
            className={`h-12 flex-[2] rounded-lg font-semibold disabled:bg-linea disabled:text-tenue ${
              sobreLimite ? "bg-aviso text-aviso-fondo" : "bg-acento text-sobre-acento"
            }`}
          >
            {fiado ? (sobreLimite ? "Fiar sobre el límite" : "Fiar") : "Confirmar venta"}{" "}
            <span className="text-xs font-normal opacity-70">{fiado ? "F12" : "Enter"}</span>
          </button>
        </div>
      </div>
    </div>
  );
}

/** Buscar al cliente por nombre, teléfono o RUT, o crearlo con nombre y teléfono sin salir del cobro. */
function ElegirCliente({ cliente, onElegir, aPagar }: { cliente: Cliente | null; onElegir: (c: Cliente | null) => void; aPagar: number }) {
  const [texto, setTexto] = useState("");
  const [resultados, setResultados] = useState<Cliente[]>([]);
  const [nuevo, setNuevo] = useState<{ nombre: string; telefono: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (cliente || nuevo) return;
    const t = setTimeout(() => {
      api
        .buscarClientes(texto, 6)
        .then(setResultados)
        .catch((e) => setError(textoError(e)));
    }, 120);
    return () => clearTimeout(t);
  }, [texto, cliente, nuevo]);

  const crear = async () => {
    if (!nuevo || !nuevo.nombre.trim()) return;
    setError(null);
    try {
      const c = await api.crearCliente({
        nombre: nuevo.nombre,
        telefono: nuevo.telefono || null,
        rut: null,
        direccion: null,
        nota: null,
        limiteCredito: null,
      });
      setNuevo(null);
      onElegir(c);
    } catch (e) {
      setError(textoError(e));
    }
  };

  if (cliente) {
    const queda = cliente.limiteCredito === null ? null : cliente.limiteCredito - cliente.deuda;
    return (
      <div className="mt-5 rounded-lg border border-linea p-4 tabular-nums">
        <div className="flex items-center justify-between gap-3">
          <div className="min-w-0">
            <div className="truncate text-lg font-semibold">{cliente.nombre}</div>
            {cliente.telefono && <div className="text-xs text-tenue">{cliente.telefono}</div>}
          </div>
          <button type="button" onClick={() => onElegir(null)} className="h-8 rounded-lg px-2 text-tenue hover:bg-fondo hover:text-texto">
            Cambiar
          </button>
        </div>
        <dl className="mt-3 space-y-1">
          <div className="flex justify-between">
            <dt className="text-tenue">Deuda actual</dt>
            <dd>{pesos(cliente.deuda)}</dd>
          </div>
          <div className="flex justify-between">
            <dt className="text-tenue">Deuda con esta compra</dt>
            <dd className="font-semibold">{pesos(cliente.deuda + aPagar)}</dd>
          </div>
          <div className="flex justify-between">
            <dt className="text-tenue">Límite de crédito</dt>
            <dd>{cliente.limiteCredito === null ? "sin límite" : `${pesos(cliente.limiteCredito)} · le quedan ${pesos(Math.max(0, queda ?? 0))}`}</dd>
          </div>
        </dl>
      </div>
    );
  }

  if (nuevo) {
    return (
      <div className="mt-5 space-y-2 rounded-lg border border-linea p-4">
        <div className="font-medium">Nuevo cliente</div>
        <input
          autoFocus
          data-cliente
          value={nuevo.nombre}
          onChange={(e) => setNuevo({ ...nuevo, nombre: e.target.value })}
          onKeyDown={(e) => e.key === "Enter" && void crear()}
          placeholder="Nombre"
          maxLength={80}
          className="h-10 w-full rounded-lg border border-linea bg-fondo px-3 outline-none focus:border-texto"
        />
        <input
          data-cliente
          value={nuevo.telefono}
          onChange={(e) => setNuevo({ ...nuevo, telefono: e.target.value })}
          onKeyDown={(e) => e.key === "Enter" && void crear()}
          placeholder="Teléfono (opcional)"
          maxLength={30}
          className="h-10 w-full rounded-lg border border-linea bg-fondo px-3 outline-none focus:border-texto"
        />
        {error && <p className="text-peligro">{error}</p>}
        <div className="flex gap-2">
          <button type="button" onClick={() => setNuevo(null)} className="h-9 flex-1 rounded-lg text-tenue hover:bg-fondo">
            Volver a buscar
          </button>
          <button
            type="button"
            onClick={() => void crear()}
            disabled={!nuevo.nombre.trim()}
            className="h-9 flex-1 rounded-lg bg-texto font-medium text-superficie disabled:bg-linea disabled:text-tenue"
          >
            Crear y elegir
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="mt-5">
      <div className="flex gap-2">
        <input
          autoFocus
          data-cliente
          value={texto}
          onChange={(e) => setTexto(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && resultados.length > 0) onElegir(resultados[0]);
          }}
          placeholder="Cliente: nombre, teléfono o RUT"
          aria-label="Buscar cliente"
          className="h-11 min-w-0 flex-1 rounded-lg border border-linea bg-fondo px-3 outline-none focus:border-texto"
        />
        <button
          type="button"
          onClick={() => setNuevo({ nombre: texto, telefono: "" })}
          className="flex h-11 items-center gap-2 rounded-lg border border-linea px-3 hover:bg-fondo"
        >
          <UserPlus size={16} aria-hidden /> Nuevo cliente
        </button>
      </div>
      {error && <p className="mt-2 text-peligro">{error}</p>}
      <ul className="mt-2 max-h-48 overflow-auto rounded-lg border border-linea">
        {resultados.length === 0 ? (
          <li className="p-3 text-center text-tenue">{texto ? "No hay clientes con ese nombre" : "Todavía no hay clientes"}</li>
        ) : (
          resultados.map((c, i) => (
            <li key={c.id}>
              <button
                type="button"
                onClick={() => onElegir(c)}
                className={`flex h-10 w-full items-center justify-between gap-3 px-3 text-left hover:bg-fondo ${i === 0 ? "bg-fondo" : ""}`}
              >
                <span className="min-w-0 truncate">
                  {c.nombre}
                  {c.telefono && <span className="text-tenue"> · {c.telefono}</span>}
                </span>
                <span className="shrink-0 text-tenue tabular-nums">{c.deuda > 0 ? `debe ${pesos(c.deuda)}` : "al día"}</span>
              </button>
            </li>
          ))
        )}
      </ul>
    </div>
  );
}
