import { Info } from "lucide-react";
import { Fragment } from "react";
import { GRUPOS_ATAJOS } from "../atajos-lista";
import Boton from "./Boton";
import Dialogo from "./Dialogo";
import Tecla from "./Tecla";

/** La lista de atajos de teclado, agrupada, con el aviso de la tecla Fn de los notebooks. */
export default function AyudaAtajos({ onCerrar }: { onCerrar: () => void }) {
  return (
    <Dialogo titulo="Atajos de teclado" onCerrar={onCerrar} ancho="w-[44rem]">
      <p className="mt-3 flex items-start gap-2 rounded-lg bg-info-fondo px-3 py-2 text-info">
        <Info size={18} className="mt-0.5 shrink-0" aria-hidden />
        ¿Las teclas F no responden en tu notebook? Prueba apretando Fn junto con la tecla, o activa el bloqueo Fn.
      </p>
      <div className="mt-4 grid grid-cols-1 gap-x-8 gap-y-5 sm:grid-cols-2">
        {GRUPOS_ATAJOS.map((g) => (
          <section key={g.titulo}>
            <h3 className="font-semibold">{g.titulo}</h3>
            <dl className="mt-2 space-y-1.5">
              {g.atajos.map((a) => (
                <div key={a.que} className="flex items-center justify-between gap-3">
                  <dt className="min-w-0">{a.que}</dt>
                  <dd className="flex shrink-0 items-center gap-1">
                    {a.teclas.map((t, i) => (
                      <Fragment key={t}>
                        {i > 0 && <span className="text-tenue">+</span>}
                        <Tecla>{t}</Tecla>
                      </Fragment>
                    ))}
                  </dd>
                </div>
              ))}
            </dl>
          </section>
        ))}
      </div>
      <div className="mt-6 flex justify-end">
        <Boton variante="primario" atajo="Esc" onClick={onCerrar}>
          Cerrar
        </Boton>
      </div>
    </Dialogo>
  );
}
