import Interruptor from "../../componentes/Interruptor";
import Boton from "../../componentes/Boton";
import { RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { api, textoError, type EstadoImpresora, type FormatoPdf, type Impresora, type TablaCaracteres } from "../../api";
import { Ajuste, Columnas, Tarjeta } from "../../componentes/Ajustes";
import Opciones, { type Opcion } from "../../componentes/Opciones";
import type { ConfigEditable } from "../../hooks/useConfig";

const ARCHIVO = "Archivo (pruebas)";

const ANCHOS: Opcion<"58" | "80">[] = [
  { id: "58", nombre: "58 mm" },
  { id: "80", nombre: "80 mm" },
];
const TABLAS: Opcion<TablaCaracteres>[] = [
  { id: "cp858", nombre: "CP858" },
  { id: "cp850", nombre: "CP850" },
  { id: "wpc1252", nombre: "WPC1252" },
];

const COPIAS: Opcion<"1" | "2">[] = [
  { id: "1", nombre: "1" },
  { id: "2", nombre: "2" },
];
const FORMATOS: Opcion<FormatoPdf>[] = [
  { id: "ticket", nombre: "Ticket 80 mm" },
  { id: "carta", nombre: "Carta" },
];
const PINES: Opcion<"2" | "5">[] = [
  { id: "2", nombre: "Pin 2" },
  { id: "5", nombre: "Pin 5" },
];


function textoEstado(e: EstadoImpresora): string {
  switch (e.tipo) {
    case "lista":
      return "lista";
    case "desconectada":
      return "desconectada";
    case "conError":
      return e.detalle.toLowerCase();
    default:
      return "sin estado";
  }
}


interface Resultado {
  error: boolean;
  texto: string;
}

/** Configuración > Impresora y escáner: impresora, boleta, PDF y cajón, con botones de prueba. */
export default function ImpresoraConfig({
  cfg,
  onCambioImpresora,
  escaner,
  soloImpresora,
}: {
  cfg: ConfigEditable;
  onCambioImpresora: () => void;
  /** Lo que sigue en la tarjeta "Boleta y escáner": la prueba del lector y las etiquetas de balanza. */
  escaner?: ReactNode;
  /** El asistente de primer arranque solo muestra la tarjeta de la impresora (con su página de prueba). */
  soloImpresora?: boolean;
}) {
  const { config, cambiar } = cfg;
  const [impresoras, setImpresoras] = useState<Impresora[] | null>(null);
  const [buscando, setBuscando] = useState(false);
  const [carpetaDefecto, setCarpetaDefecto] = useState("");
  const [carpetaPruebas, setCarpetaPruebas] = useState("");
  const [resultados, setResultados] = useState<Record<string, Resultado | "trabajando">>({});

  const actualizar = useCallback(async () => {
    setBuscando(true);
    try {
      setImpresoras(await api.listarImpresoras());
    } catch (e) {
      setResultados((r) => ({ ...r, lista: { error: true, texto: textoError(e) } }));
    } finally {
      setBuscando(false);
    }
  }, []);

  useEffect(() => {
    void actualizar();
    api.carpetaPdfPredeterminada().then(setCarpetaDefecto).catch(() => {});
    api.carpetaImpresoraPruebas().then(setCarpetaPruebas).catch(() => {});
  }, [actualizar]);

  /** Ejecuta una prueba y deja el resultado junto a su botón. */
  const probar = async (clave: string, accion: () => Promise<unknown>, exito: string) => {
    setResultados((r) => ({ ...r, [clave]: "trabajando" }));
    try {
      await accion();
      setResultados((r) => ({ ...r, [clave]: { error: false, texto: exito } }));
    } catch (e) {
      setResultados((r) => ({ ...r, [clave]: { error: true, texto: textoError(e) } }));
    }
    onCambioImpresora();
  };

  const cambiarImpresora = async (nombre: string) => {
    await cambiar({ impresora: nombre || null });
    onCambioImpresora();
  };

  if (!config) return <p className="text-tenue">Cargando…</p>;
  const noEncontrada = config.impresora !== null && impresoras !== null && !impresoras.some((i) => i.nombre === config.impresora);

  const izquierda = (
        <Tarjeta titulo="Impresora" ayuda="La térmica instalada en Windows y cómo imprime.">
          <div className="flex gap-2">
            <select
              value={config.impresora ?? ""}
              onChange={(e) => void cambiarImpresora(e.target.value)}
              aria-label="Impresora"
              className="h-11 min-w-0 flex-1 rounded-lg border border-borde-control px-3 focus:border-acento"
            >
              <option value="">{impresoras === null ? "Buscando impresoras…" : "Elige una impresora…"}</option>
              {impresoras?.map((i) => (
                <option key={i.nombre} value={i.nombre}>
                  {i.nombre}
                  {i.predeterminada ? " (predeterminada)" : ""}
                  {i.esVirtual ? " · escribe un .bin" : ` · ${textoEstado(i.estado)}`}
                </option>
              ))}
              {noEncontrada && <option value={config.impresora ?? ""}>{config.impresora} (no está instalada)</option>}
            </select>
            <Boton icono={RefreshCw} onClick={() => void actualizar()} disabled={buscando}>
              Actualizar
            </Boton>
          </div>
          <Nota resultado={resultados.lista} />
          {config.impresora === ARCHIVO && (
            <p className="text-xs text-tenue">
              Impresora de pruebas: cada trabajo queda como un archivo .bin en <span className="break-all select-text">{carpetaPruebas}</span>
            </p>
          )}
          <Ajuste etiqueta="Ancho del papel" ayuda="58 mm: 32 caracteres por línea. 80 mm: 48.">
            <Opciones
              nombre="ancho"
              etiqueta="Ancho del papel"
              opciones={ANCHOS}
              valor={String(config.anchoPapelMm) as "58" | "80"}
              onCambiar={(v) => void cambiar({ anchoPapelMm: v === "58" ? 58 : 80 })}
            />
          </Ajuste>
          <Ajuste etiqueta="Tabla de caracteres" ayuda="CP858 funciona en la mayoría. Si las tildes salen mal, prueba otra.">
            <Opciones
              nombre="tabla"
              etiqueta="Tabla de caracteres"
              opciones={TABLAS}
              valor={config.tablaCaracteres}
              onCambiar={(v) => void cambiar({ tablaCaracteres: v })}
            />
          </Ajuste>
          <Ajuste etiqueta="Imprimir al cobrar" ayuda="La boleta sale sola al confirmar cada venta.">
            <Interruptor
              etiqueta="Imprimir al cobrar"
              marcado={config.imprimirAlCobrar}
              onCambiar={(v) => void cambiar({ imprimirAlCobrar: v })}
            />
          </Ajuste>
          <Ajuste etiqueta="Copias" ayuda="Boletas por venta al cobrar. Reimprimir saca siempre una.">
            <Opciones
              nombre="copias"
              etiqueta="Copias"
              opciones={COPIAS}
              valor={String(config.copias) as "1" | "2"}
              onCambiar={(v) => void cambiar({ copias: v === "2" ? 2 : 1 })}
            />
          </Ajuste>
          <div className="flex flex-wrap items-center gap-3">
            <Boton
              disabled={!config.impresora || resultados.prueba === "trabajando"}
              onClick={() => void probar("prueba", api.imprimirPrueba, "Página de prueba enviada.")}
            >
              Imprimir página de prueba
            </Boton>
            <span className="text-[13px] text-tenue">Si las tildes salen mal, prueba otra tabla de caracteres.</span>
          </div>
          <Nota resultado={resultados.prueba} />

          <Seccion titulo="Cajón de dinero">
            <Ajuste etiqueta="Abrir el cajón al cobrar" ayuda="El cajón va conectado a la impresora; se abre con un pulso.">
              <Interruptor
                etiqueta="Abrir el cajón al cobrar"
                marcado={config.cajonActivo}
                onCambiar={(v) => void cambiar({ cajonActivo: v })}
              />
            </Ajuste>
            <Ajuste etiqueta="Conector" ayuda="Casi todos usan el pin 2. Si no abre, prueba el 5.">
              <Opciones
                nombre="pin"
                etiqueta="Pin del cajón"
                opciones={PINES}
                valor={String(config.cajonPin) as "2" | "5"}
                onCambiar={(v) => void cambiar({ cajonPin: v === "5" ? 5 : 2 })}
              />
            </Ajuste>
            <Ajuste etiqueta="Abrir solo en efectivo" ayuda="Con tarjeta o transferencia el cajón queda cerrado.">
              <Interruptor
                etiqueta="Abrir solo en efectivo"
                marcado={config.cajonSoloEfectivo}
                onCambiar={(v) => void cambiar({ cajonSoloEfectivo: v })}
              />
            </Ajuste>
            <div className="flex flex-wrap items-center gap-3">
              <Boton
                disabled={!config.impresora || resultados.cajon === "trabajando"}
                onClick={() => void probar("cajon", api.abrirCajon, "Pulso enviado: el cajón debería abrirse.")}
              >
                Probar cajón
              </Boton>
              <Nota resultado={resultados.cajon} />
            </div>
          </Seccion>
        </Tarjeta>
  );
  if (soloImpresora) return izquierda;
  const derecha = (
        <Tarjeta titulo="Boleta y escáner" ayuda="Qué lleva la boleta, dónde se guarda el PDF y cómo lee el lector.">
          <Seccion titulo="Boleta">
            <Ajuste etiqueta="Leyenda al pie" ayuda="«Comprobante interno sin validez tributaria».">
              <Interruptor
                etiqueta="Leyenda al pie"
                marcado={config.boletaLeyenda}
                onCambiar={(v) => void cambiar({ boletaLeyenda: v })}
              />
            </Ajuste>
            <Ajuste etiqueta="Código de barras" ayuda="CODE128 con el folio, para buscar la venta más adelante.">
              <Interruptor
                etiqueta="Código de barras"
                marcado={config.boletaCodigo}
                onCambiar={(v) => void cambiar({ boletaCodigo: v })}
              />
            </Ajuste>
          </Seccion>

          <Seccion titulo="PDF">
            <Ajuste etiqueta="Formato" ayuda="Ticket: 80 mm de ancho, del largo de la boleta. Carta: para impresoras comunes.">
              <Opciones
                nombre="pdf"
                etiqueta="Formato del PDF"
                opciones={FORMATOS}
                valor={config.pdfFormato}
                onCambiar={(v) => void cambiar({ pdfFormato: v })}
              />
            </Ajuste>
            <div>
              <div className="text-sm font-medium">Carpeta</div>
              <div className="mt-1 flex flex-wrap items-center gap-2">
                <span className="min-w-0 flex-1 break-all text-tenue select-text">{config.pdfCarpeta ?? carpetaDefecto}</span>
                <Boton
                  tamano="chico"
                  onClick={() =>
                    void api.elegirCarpetaPdf().then((c) => {
                      if (c) void cambiar({ pdfCarpeta: c });
                    })
                  }
                >
                  Cambiar…
                </Boton>
                {config.pdfCarpeta && (
                  <Boton variante="terciario" tamano="chico" onClick={() => void cambiar({ pdfCarpeta: null })}>
                    Usar la predeterminada
                  </Boton>
                )}
              </div>
            </div>
          </Seccion>
          {escaner}
        </Tarjeta>
  );
  return <Columnas izquierda={izquierda} derecha={derecha} />;
}

export function Seccion({ titulo, children }: { titulo: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-4 border-t border-linea pt-4 first:border-t-0 first:pt-0">
      <h3 className="text-xs font-semibold text-tenue">{titulo}</h3>
      {children}
    </section>
  );
}

function Nota({ resultado }: { resultado: Resultado | "trabajando" | undefined }) {
  if (!resultado) return null;
  if (resultado === "trabajando") return <p className="text-tenue">Enviando…</p>;
  return (
    <p role={resultado.error ? "alert" : "status"} className={resultado.error ? "text-peligro" : "text-exito"}>
      {resultado.texto}
    </p>
  );
}
