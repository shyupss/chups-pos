import { PanelLeftClose, PanelLeftOpen } from "lucide-react";
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
}

interface Insignia {
  valor: number;
  /** Texto para lectores de pantalla y para el tooltip con la barra colapsada. */
  descripcion: string;
}

export default function BarraLateral({ actual, expandida, onIr, onAlternar, insignias = {} }: Props) {
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
        className={`flex h-14 shrink-0 items-center font-semibold tracking-wide ${expandida ? "px-5" : "justify-center"}`}
        title="CHUPS POS"
      >
        {expandida ? "CHUPS POS" : "CP"}
      </div>

      <nav aria-label="Secciones" className="flex min-h-0 flex-1 flex-col overflow-y-auto px-2 pb-2">
        <ul className="space-y-1">{SECCIONES.filter((s) => !s.abajo).map(item)}</ul>
        <ul className="mt-auto space-y-1 border-t border-linea pt-2">{SECCIONES.filter((s) => s.abajo).map(item)}</ul>
      </nav>

      <div className={`flex shrink-0 items-center gap-2 border-t border-linea p-2 ${expandida ? "" : "flex-col"}`}>
        {/* Fijo por ahora: se conecta con los turnos de caja en la fase 4. */}
        <div
          className={`flex h-8 items-center gap-2 rounded-lg bg-exito-fondo text-xs font-medium text-exito ${
            expandida ? "flex-1 px-3" : "w-full justify-center"
          }`}
          title="Caja abierta"
        >
          <span className="size-2 shrink-0 rounded-full bg-exito" aria-hidden />
          <span className={expandida ? "truncate" : "sr-only"}>Caja abierta</span>
        </div>
        <button
          type="button"
          onClick={onAlternar}
          title={accion}
          aria-label={accion}
          className="flex size-8 shrink-0 items-center justify-center rounded-lg text-tenue hover:bg-fondo hover:text-texto"
        >
          {expandida ? <PanelLeftClose size={18} aria-hidden /> : <PanelLeftOpen size={18} aria-hidden />}
        </button>
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
        activa ? "bg-acento/12 font-medium text-acento" : "text-texto hover:bg-fondo"
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
