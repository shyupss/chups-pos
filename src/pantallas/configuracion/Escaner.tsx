import { useEffect, useRef, useState } from "react";
import { api, textoError, type FormatoBalanza } from "../../api";
import { Fila } from "../../componentes/Ajustes";
import Opciones, { type Opcion } from "../../componentes/Opciones";
import { cantidad, pesos } from "../../formato";
import type { ConfigEditable } from "../../hooks/useConfig";
import { Seccion } from "./Impresora";

/** Un lector escribe cada carácter a pocos milisegundos del anterior; una persona, mucho más lento. */
const MS_LECTOR = 50;

const SI_NO: Opcion<"si" | "no">[] = [
  { id: "si", nombre: "Sí" },
  { id: "no", nombre: "No" },
];
const FORMATOS: Opcion<FormatoBalanza>[] = [
  { id: "peso", nombre: "Peso en gramos" },
  { id: "precio", nombre: "Precio en pesos" },
];
const LARGOS: Opcion<"4" | "5" | "6">[] = [
  { id: "4", nombre: "4" },
  { id: "5", nombre: "5" },
  { id: "6", nombre: "6" },
];

const claseCampo = "h-10 w-full rounded-lg border border-linea bg-fondo px-3 outline-none focus:border-texto";

interface Lectura {
  codigo: string;
  ms: number;
  promedio: number;
  lector: boolean;
}

/** Configuración > Impresora y escáner: prueba del lector y códigos de balanza. */
export default function EscanerConfig({ cfg }: { cfg: ConfigEditable }) {
  const { config, cambiar } = cfg;
  return (
    <div className="mt-6 space-y-6 border-t border-linea pt-4">
      <Seccion titulo="Escáner">
        <PruebaEscaner />
      </Seccion>
      {config && (
        <Seccion titulo="Códigos de balanza">
          <p className="text-tenue">
            Para las etiquetas que imprime la balanza: un EAN-13 que empieza con 20 a 29, el código del producto y el peso
            (o el precio). La Caja agrega el producto por kg con ese peso, sin pedirlo.
          </p>
          <Fila etiqueta="Leer etiquetas de balanza" ayuda="Si el código no existe como producto y tiene ese formato.">
            <Opciones
              nombre="balanza"
              etiqueta="Leer etiquetas de balanza"
              opciones={SI_NO}
              valor={config.balanzaActiva ? "si" : "no"}
              onCambiar={(v) => void cambiar({ balanzaActiva: v === "si" })}
            />
          </Fila>
          <Fila etiqueta="Prefijos" ayuda="Dos dígitos entre 20 y 29: un rango (20-29) o una lista (20,21,25).">
            <Prefijos valor={config.balanzaPrefijos} onGuardar={(v) => void cambiar({ balanzaPrefijos: v })} />
          </Fila>
          <Fila etiqueta="La etiqueta trae" ayuda="Si trae precio, el peso se calcula con el precio por kg del producto.">
            <Opciones
              nombre="formato-balanza"
              etiqueta="La etiqueta trae"
              opciones={FORMATOS}
              valor={config.balanzaFormato}
              onCambiar={(v) => void cambiar({ balanzaFormato: v })}
            />
          </Fila>
          <Fila etiqueta="Dígitos del código de producto" ayuda="Los que siguen al prefijo; el resto, hasta el último, es el valor.">
            <Opciones
              nombre="largo-balanza"
              etiqueta="Dígitos del código de producto"
              opciones={LARGOS}
              valor={String(config.balanzaLargoCodigo) as "4" | "5" | "6"}
              onCambiar={(v) => void cambiar({ balanzaLargoCodigo: Number(v) })}
            />
          </Fila>
          <Ejemplo largo={config.balanzaLargoCodigo} formato={config.balanzaFormato} />
          <PruebaBalanza activa={config.balanzaActiva} />
        </Seccion>
      )}
    </div>
  );
}

/** Mide el tiempo entre teclas para saber si el código lo escribió un lector o una persona. */
function PruebaEscaner() {
  const [texto, setTexto] = useState("");
  const [lectura, setLectura] = useState<Lectura | null>(null);
  const tiempos = useRef<number[]>([]);

  return (
    <div className="space-y-2">
      <input
        value={texto}
        onChange={(e) => setTexto(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            const t = tiempos.current;
            if (texto.trim() && t.length > 0) {
              const ms = Math.round(performance.now() - t[0]);
              const promedio = t.length > 1 ? Math.round((t[t.length - 1] - t[0]) / (t.length - 1)) : ms;
              setLectura({ codigo: texto.trim(), ms, promedio, lector: t.length >= 4 && promedio < MS_LECTOR });
            }
            tiempos.current = [];
            setTexto("");
          } else if (e.key.length === 1) {
            if (texto === "") tiempos.current = [];
            tiempos.current.push(performance.now());
          }
        }}
        placeholder="Escanea cualquier código"
        aria-label="Escanea cualquier código"
        className={claseCampo}
        autoComplete="off"
        spellCheck={false}
      />
      {lectura && (
        <p role="status" className={lectura.lector ? "text-exito" : "text-aviso"}>
          <span className="font-medium select-text">{lectura.codigo}</span> · {lectura.codigo.length} caracteres en{" "}
          {lectura.ms} ms ({lectura.promedio} ms entre teclas):{" "}
          {lectura.lector
            ? "lo escribió un lector de códigos."
            : `parece tecleo manual (un lector tarda menos de ${MS_LECTOR} ms entre teclas).`}
        </p>
      )}
      <p className="text-xs text-tenue">
        El lector debe terminar cada código con Enter. Si no lo hace, configúralo con las hojas de códigos de su manual.
      </p>
    </div>
  );
}

/** Se guarda al salir del campo o con Enter, para no validar a medio escribir. */
function Prefijos({ valor, onGuardar }: { valor: string; onGuardar: (v: string) => void }) {
  const [texto, setTexto] = useState(valor);
  useEffect(() => setTexto(valor), [valor]);
  return (
    <input
      value={texto}
      onChange={(e) => setTexto(e.target.value)}
      onBlur={() => texto !== valor && onGuardar(texto)}
      onKeyDown={(e) => e.key === "Enter" && texto !== valor && onGuardar(texto)}
      aria-label="Prefijos de balanza"
      className="h-10 w-32 rounded-lg border border-linea bg-fondo px-3 tabular-nums outline-none focus:border-texto"
    />
  );
}

/** Cómo se lee una etiqueta con la configuración actual. */
function Ejemplo({ largo, formato }: { largo: number; formato: FormatoBalanza }) {
  const codigo = "12345678".slice(0, largo);
  const valor = (formato === "peso" ? "532" : "1224").padStart(10 - largo, "0");
  return (
    <p className="rounded-lg bg-fondo px-3 py-2 text-xs tabular-nums">
      <span className="text-tenue">Ejemplo: </span>
      <span className="font-medium">20</span> <span className="font-medium">{codigo}</span>{" "}
      <span className="font-medium">{valor}</span> <span className="text-tenue">+ dígito verificador</span>
      <span className="text-tenue"> → producto con código </span>
      {codigo}, {formato === "peso" ? "532 g" : "$1.224 (el peso se calcula con el precio por kg)"}
    </p>
  );
}

/** Lee una etiqueta real como lo haría la Caja. */
function PruebaBalanza({ activa }: { activa: boolean }) {
  const [texto, setTexto] = useState("");
  const [resultado, setResultado] = useState<{ error: boolean; texto: string } | null>(null);

  const probar = async () => {
    const codigo = texto.trim();
    if (!codigo) return;
    try {
      const l = await api.leerCodigoBalanza(codigo);
      setResultado(
        l
          ? {
              error: false,
              texto: `${l.producto.nombre}: ${cantidad(l.cantidadMil, "kg")} a ${pesos(l.producto.precioVenta)}/kg`,
            }
          : {
              error: true,
              texto: activa
                ? "No es una etiqueta de balanza con esta configuración (revisa los prefijos y el largo)."
                : "Activa «Leer etiquetas de balanza» para probar.",
            },
      );
    } catch (e) {
      setResultado({ error: true, texto: textoError(e) });
    }
  };

  return (
    <div className="space-y-2">
      <input
        value={texto}
        onChange={(e) => setTexto(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            void probar();
          }
        }}
        placeholder="Escanea una etiqueta de la balanza para probar"
        aria-label="Probar una etiqueta de balanza"
        className={claseCampo}
        autoComplete="off"
        spellCheck={false}
      />
      {resultado && (
        <p role="status" className={resultado.error ? "text-peligro" : "text-exito"}>
          {resultado.texto}
        </p>
      )}
    </div>
  );
}
