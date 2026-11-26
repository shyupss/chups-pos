import { X } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { api, textoError, type Proveedor } from "../api";
import Boton from "./Boton";
import { ofrecerAgregar } from "./compra-nucleo";

/**
 * El proveedor de una compra, al hilo: se escribe y la lista se filtra, y al final ofrece
 * "+ Agregar proveedor «texto»", que pide solo el nombre (y el teléfono, si se quiere) en el mismo lugar.
 * El resto de sus datos se completa después en Inventario > Proveedores. Sin proveedor también vale.
 */
export default function CampoProveedor({
  valor,
  onElegir,
}: {
  valor: Proveedor | null;
  onElegir: (p: Proveedor | null) => void;
}) {
  const id = useId();
  const [texto, setTexto] = useState("");
  const [abierta, setAbierta] = useState(false);
  const [lista, setLista] = useState<Proveedor[]>([]);
  const [resaltada, setResaltada] = useState(0);
  const [nuevo, setNuevo] = useState<{ nombre: string; telefono: string } | null>(null);
  const [guardando, setGuardando] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const campo = useRef<HTMLInputElement>(null);

  // Se pide al backend lo que coincide con lo escrito, sin tildes ni mayúsculas, cuando se deja de escribir.
  useEffect(() => {
    if (!abierta) return;
    let vigente = true;
    const t = setTimeout(() => {
      api
        .listarProveedores(texto.trim(), false, 8, 0)
        .then((p) => vigente && setLista(p))
        .catch(() => vigente && setLista([]));
    }, 120);
    return () => {
      vigente = false;
      clearTimeout(t);
    };
  }, [texto, abierta]);

  const agregar = ofrecerAgregar(lista, texto);
  // Las opciones: los proveedores que coinciden y, al final, agregar el que se escribió.
  const opciones: { clave: string; texto: string; agregar?: true; proveedor?: Proveedor }[] = [
    ...lista.map((p) => ({ clave: `p${p.id}`, texto: p.nombre, proveedor: p })),
    ...(agregar ? [{ clave: "nuevo", texto: `+ Agregar proveedor «${texto.trim()}»`, agregar: true as const }] : []),
  ];

  const elegir = (p: Proveedor) => {
    onElegir(p);
    setTexto("");
    setAbierta(false);
  };

  const guardarNuevo = async () => {
    if (!nuevo || guardando) return;
    const nombre = nuevo.nombre.trim();
    if (!nombre) return setError("Escribe el nombre del proveedor.");
    setGuardando(true);
    setError(null);
    try {
      const p = await api.crearProveedor({ nombre, rut: null, telefono: nuevo.telefono.trim() || null, contacto: null, nota: null });
      setNuevo(null);
      elegir(p);
    } catch (e) {
      setError(textoError(e));
    } finally {
      setGuardando(false);
    }
  };

  if (nuevo) {
    return (
      <div className="rounded-lg border border-borde-control p-3">
        <p className="text-sm font-medium">Proveedor nuevo</p>
        <div className="mt-2 grid gap-3 sm:grid-cols-[1fr_14rem]">
          <label className="block">
            <span className="text-sm text-tenue">Nombre</span>
            <input
              autoFocus
              value={nuevo.nombre}
              onChange={(e) => setNuevo({ ...nuevo, nombre: e.target.value })}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  void guardarNuevo();
                } else if (e.key === "Escape") {
                  e.preventDefault();
                  e.stopPropagation();
                  setNuevo(null);
                }
              }}
              maxLength={80}
              className="mt-1 h-11 w-full rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento"
            />
          </label>
          <label className="block">
            <span className="text-sm text-tenue">Teléfono (opcional)</span>
            <input
              value={nuevo.telefono}
              onChange={(e) => setNuevo({ ...nuevo, telefono: e.target.value })}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  void guardarNuevo();
                }
              }}
              maxLength={30}
              inputMode="tel"
              className="mt-1 h-11 w-full rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento"
            />
          </label>
        </div>
        {error && <p className="mt-2 text-peligro">{error}</p>}
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <Boton variante="primario" onClick={() => void guardarNuevo()} disabled={guardando || !nuevo.nombre.trim()}>
            Guardar proveedor
          </Boton>
          <Boton
            variante="terciario"
            onClick={() => {
              setNuevo(null);
              setError(null);
              campo.current?.focus();
            }}
          >
            Cancelar
          </Boton>
          <span className="text-sm text-tenue">Los demás datos los completas después en Inventario &gt; Proveedores.</span>
        </div>
      </div>
    );
  }

  if (valor) {
    return (
      <div className="flex h-11 items-center justify-between gap-3 rounded-lg border border-borde-control bg-superficie px-3">
        <span className="min-w-0 truncate font-medium">{valor.nombre}</span>
        <Boton variante="terciario" tamano="chico" icono={X} onClick={() => onElegir(null)}>
          Cambiar
        </Boton>
      </div>
    );
  }

  return (
    <div className="relative">
      <input
        ref={campo}
        role="combobox"
        aria-expanded={abierta}
        aria-controls={`${id}-lista`}
        aria-activedescendant={abierta && opciones[resaltada] ? `${id}-${opciones[resaltada].clave}` : undefined}
        aria-label="Proveedor"
        autoComplete="off"
        value={texto}
        placeholder="Escribe para buscar o agregar (puedes dejarlo vacío)"
        onFocus={() => setAbierta(true)}
        onBlur={() => setTimeout(() => setAbierta(false), 150)}
        onChange={(e) => {
          setTexto(e.target.value);
          setResaltada(0);
          setAbierta(true);
        }}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" || e.key === "ArrowUp") {
            e.preventDefault();
            setAbierta(true);
            setResaltada((i) => Math.max(0, Math.min(opciones.length - 1, i + (e.key === "ArrowDown" ? 1 : -1))));
          } else if (e.key === "Enter" && abierta && opciones[resaltada]) {
            e.preventDefault();
            const o = opciones[resaltada];
            if (o.proveedor) elegir(o.proveedor);
            else setNuevo({ nombre: texto.trim(), telefono: "" });
          } else if (e.key === "Escape" && abierta) {
            // Cierra la lista y no la compra.
            e.preventDefault();
            e.stopPropagation();
            setAbierta(false);
          }
        }}
        className="h-11 w-full rounded-lg border border-borde-control bg-superficie px-3 focus:border-acento"
      />
      {abierta && opciones.length > 0 && (
        <ul id={`${id}-lista`} role="listbox" className="absolute z-20 mt-1 max-h-72 w-full overflow-auto tarjeta shadow-s2">
          {opciones.map((o, i) => (
            <li
              key={o.clave}
              id={`${id}-${o.clave}`}
              role="option"
              aria-selected={i === resaltada}
              // mouseDown (no click): se resuelve antes de que el campo pierda el foco y cierre la lista.
              onMouseDown={(e) => {
                e.preventDefault();
                if (o.proveedor) elegir(o.proveedor);
                else setNuevo({ nombre: texto.trim(), telefono: "" });
              }}
              onMouseEnter={() => setResaltada(i)}
              className={`flex h-11 cursor-pointer items-center gap-2 px-3 ${i === resaltada ? "bg-seleccion" : ""} ${
                o.agregar ? "font-medium text-acento" : ""
              }`}
            >
              <span className="truncate">{o.texto}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
