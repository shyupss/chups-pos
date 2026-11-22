import Boton from "./Boton";
import { PanelLeftClose, PanelLeftOpen, Printer } from "lucide-react";
import type { EstadoConfigurada, EstadoLicencia, Turno } from "../api";
import { fechaHora, horaODia } from "../formato";
import { SECCIONES, type DatosSeccion, type Seccion } from "../secciones";

/** "auto": colapsada en la Caja y expandida en las demás secciones. */
export type ModoBarra = "auto" | "expandida" | "colapsada";

interface Props {
  actual: Seccion;
  expandida: boolean;
  onIr: (seccion: Seccion) => void;
  /** Fija la barra en el estado contrario al actual. */
  onAlternar: () => void;
  /** Número junto a una sección, por ejemplo las ventas en espera de la Caja. */
  insignias?: Partial<Record<Seccion, Insignia>>;
  /** Estado de la impresora elegida; null mientras se lee. */
  impresora?: EstadoConfigurada | null;
  /** Turno abierto; null si la caja está cerrada y undefined mientras se lee. */
  turno?: Turno | null;
  /** En prueba, gracia o consulta se muestra una línea discreta que lleva a Configuración > Licencia. */
  licencia?: EstadoLicencia | null;
  onLicencia?: () => void;
}

interface Insignia {
  valor: number;
  /** Texto para lectores de pantalla y para el tooltip con la barra colapsada. */
  descripcion: string;
}

export default function BarraLateral({
  actual,
  expandida,
  onIr,
  onAlternar,
  insignias = {},
  impresora = null,
  turno,
  licencia = null,
  onLicencia,
}: Props) {
  const item = (s: DatosSeccion) => (
    <li key={s.id}>
      <Item seccion={s} activa={s.id === actual} expandida={expandida} insignia={insignias[s.id]} onIr={onIr} />
    </li>
  );
  const accion = expandida ? "Fijar colapsada" : "Fijar expandida";

  return (
    <aside
      className={`flex shrink-0 flex-col border-r border-linea bg-superficie ${expandida ? "w-52" : "w-16"}`}
    >
      <div
        className={`flex h-14 shrink-0 items-center font-semibold ${expandida ? "px-5" : "justify-center"}`}
        title="CHUPS POS"
      >
        {expandida ? "CHUPS POS" : "CP"}
      </div>

      <nav aria-label="Secciones" className="flex min-h-0 flex-1 flex-col overflow-y-auto px-2 pb-2">
        <ul className="space-y-1">{SECCIONES.filter((s) => !s.abajo).map(item)}</ul>
        <ul className="mt-auto space-y-1 border-t border-linea pt-2">{SECCIONES.filter((s) => s.abajo).map(item)}</ul>
      </nav>

      <Estados
        turno={turno}
        impresora={impresora}
        licencia={licencia}
        expandida={expandida}
        onIr={onIr}
        onLicencia={onLicencia}
      />
      <div className={`flex shrink-0 items-center gap-2 px-2 pb-2 ${expandida ? "justify-end" : "flex-col"}`}>
        <Boton tamano="icono" onClick={onAlternar} title={accion} aria-label={accion}>
          {expandida ? <PanelLeftClose size={18} aria-hidden /> : <PanelLeftOpen size={18} aria-hidden />}
        </Boton>
      </div>
    </aside>
  );
}

function Item({
  seccion: { id, nombre, tecla, Icono },
  activa,
  expandida,
  insignia,
  onIr,
}: {
  seccion: DatosSeccion;
  activa: boolean;
  expandida: boolean;
  insignia?: Insignia;
  onIr: (seccion: Seccion) => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onIr(id)}
      aria-current={activa ? "page" : undefined}
      aria-keyshortcuts={tecla}
      title={expandida ? undefined : `${nombre} (${tecla})${insignia ? ` · ${insignia.descripcion}` : ""}`}
      className={`relative flex h-10 w-full items-center gap-3 rounded-lg ${expandida ? "px-3" : "justify-center"} ${
        activa ? "bg-seleccion font-semibold text-acento" : "text-texto hover:bg-suave"
      }`}
    >
      <Icono size={20} className="shrink-0" aria-hidden />
      <span className={expandida ? "flex-1 truncate text-left" : "sr-only"}>{nombre}</span>
      {insignia && (
        <>
          <span
            aria-hidden
            className={`h-5 min-w-5 rounded-full bg-aviso px-1.5 text-center text-xs font-semibold leading-5 text-aviso-fondo tabular-nums ${
              expandida ? "" : "absolute top-0 right-0.5"
            }`}
          >
            {insignia.valor}
          </span>
          <span className="sr-only">, {insignia.descripcion}</span>
        </>
      )}
      {expandida && <span className={`text-xs ${activa ? "" : "text-tenue"}`}>{tecla}</span>}
    </button>
  );
}

type Nivel = "ok" | "error" | "aviso" | "info" | "neutro";

const PUNTO: Record<Nivel, string> = {
  ok: "bg-exito",
  error: "bg-peligro",
  aviso: "bg-aviso",
  info: "bg-info",
  neutro: "bg-tenue",
};

/** La línea de la licencia: nada si está activa. */
function estadoLicencia(l: EstadoLicencia) {
  const dias = l.diasRestantes === 1 ? "1 día" : `${l.diasRestantes ?? 0} días`;
  switch (l.modo) {
    case "activa":
      return null;
    case "prueba":
      return {
        texto: `Prueba: quedan ${dias}`,
        nivel: "info" as Nivel,
        detalle: `Prueba de CHUPS POS: quedan ${dias}. Activa la licencia en Configuración > Licencia.`,
      };
    case "gracia":
      return {
        texto: `Gracia: quedan ${dias}`,
        nivel: "aviso" as Nivel,
        detalle: `La prueba terminó: quedan ${dias} para activar la licencia.`,
      };
    case "consulta":
      return {
        texto: "Modo consulta",
        nivel: "error" as Nivel,
        detalle: "La prueba terminó: activa la licencia para volver a vender.",
      };
  }
}

/** Lo que dice la barra de la caja y de la impresora: el texto, su nivel y el detalle para el tooltip. */
function estadoCaja(turno: Turno | null) {
  return {
    texto: turno ? `Caja abierta desde ${horaODia(turno.abiertoEn)}` : "Caja cerrada",
    nivel: (turno ? "ok" : "neutro") as Nivel,
    detalle: turno ? `Caja abierta desde el ${fechaHora(turno.abiertoEn)} (turno N° ${turno.id})` : "Caja cerrada: ábrela en la Caja",
  };
}

function estadoImpresora(impresora: EstadoConfigurada) {
  const e = impresora.estado;
  const [texto, nivel]: [string, Nivel] =
    impresora.impresora === null
      ? ["Sin impresora", "neutro"]
      : e?.tipo === "lista"
        ? ["Impresora lista", "ok"]
        : e?.tipo === "desconectada"
          ? ["Impresora desconectada", "error"]
          : e?.tipo === "conError"
            ? [`Impresora: ${e.detalle}`, "error"]
            : ["Impresora sin estado", "neutro"];
  const detalle = impresora.impresora ? `${texto} (${impresora.impresora})` : `${texto}: elígela en Configuración`;
  return { texto, nivel, detalle };
}

/**
 * Caja e impresora al pie de la barra. Expandida: un punto y su texto por línea (cada una lleva a donde se
 * arregla). Colapsada: un solo ícono de impresora con un punto (verde si todo anda, rojo si hay un error)
 * y el detalle completo en el tooltip y en el aria-label.
 */
function Estados({
  turno,
  impresora,
  licencia,
  expandida,
  onIr,
  onLicencia,
}: {
  turno: Turno | null | undefined;
  impresora: EstadoConfigurada | null;
  licencia: EstadoLicencia | null;
  expandida: boolean;
  onIr: (seccion: Seccion) => void;
  onLicencia?: () => void;
}) {
  const caja = turno === undefined ? null : estadoCaja(turno);
  const imp = impresora ? estadoImpresora(impresora) : null;
  const lic = licencia ? estadoLicencia(licencia) : null;
  if (!caja && !imp && !lic) return null;

  if (!expandida) {
    const nivel: Nivel =
      imp?.nivel === "error" || lic?.nivel === "error"
        ? "error"
        : lic?.nivel === "aviso"
          ? "aviso"
          : imp?.nivel === "ok"
            ? "ok"
            : "neutro";
    const resumen = [caja?.texto, imp?.texto.toLowerCase(), lic?.texto.toLowerCase()].filter(Boolean).join(", ");
    return (
      <div className="flex shrink-0 justify-center border-t border-linea p-2">
        <Boton variante="terciario" tamano="icono" onClick={() => onIr("caja")} title={resumen} aria-label={resumen} className="relative">
          <Printer size={18} aria-hidden />
          <span aria-hidden className={`absolute top-1 right-1 size-2 rounded-full ring-2 ring-superficie ${PUNTO[nivel]}`} />
        </Boton>
      </div>
    );
  }

  const linea = (e: { texto: string; nivel: Nivel; detalle: string }, destino: Seccion) => (
    <Boton key={e.texto} variante="terciario" tamano="chico" bloque onClick={() => onIr(destino)} title={e.detalle} className="text-xs">
      <span aria-hidden className={`size-2 shrink-0 rounded-full ${PUNTO[e.nivel]}`} />
      <span className="truncate">{e.texto}</span>
    </Boton>
  );
  return (
    <div className="flex shrink-0 flex-col gap-0.5 border-t border-linea p-2">
      {caja && linea(caja, "caja")}
      {imp && linea(imp, "configuracion")}
      {lic && (
        <Boton
          variante="terciario"
          tamano="chico"
          bloque
          onClick={() => (onLicencia ? onLicencia() : onIr("configuracion"))}
          title={lic.detalle}
          className="text-xs"
        >
          <span aria-hidden className={`size-2 shrink-0 rounded-full ${PUNTO[lic.nivel]}`} />
          <span className="truncate">{lic.texto}</span>
        </Boton>
      )}
    </div>
  );
}
