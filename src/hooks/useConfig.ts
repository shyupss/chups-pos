import { useCallback, useEffect, useRef, useState } from "react";
import { api, textoError, type Config } from "../api";
import { esCancelado, type ConPin } from "../componentes/PedirPin";

/**
 * Configuración de la tabla config para la pantalla de Configuración.
 * `cambiar` muestra el cambio al instante y lo guarda (pidiendo el PIN si "cambiar configuración"
 * está protegida); si Rust lo rechaza o se cancela el PIN, vuelve atrás.
 */
export function useConfig(conPin: ConPin) {
  const [config, setConfig] = useState<Config | null>(null);
  const [error, setError] = useState<string | null>(null);
  const ultima = useRef<Config | null>(null);

  useEffect(() => {
    api
      .obtenerConfig()
      .then((c) => {
        ultima.current = c;
        setConfig(c);
      })
      .catch((e) => setError(textoError(e)));
  }, []);

  const cambiar = useCallback(
    async (parcial: Partial<Config>) => {
      const anterior = ultima.current;
      if (!anterior) return;
      const nueva = { ...anterior, ...parcial };
      ultima.current = nueva;
      setConfig(nueva);
      setError(null);
      try {
        const guardada = await conPin((pin) => api.guardarConfig(nueva, pin), { accion: "Cambiar la configuración" });
        ultima.current = guardada;
        setConfig(guardada);
      } catch (e) {
        ultima.current = anterior;
        setConfig(anterior);
        if (!esCancelado(e)) setError(textoError(e));
      }
    },
    [conPin],
  );

  return { config, error, cambiar };
}

export type ConfigEditable = ReturnType<typeof useConfig>;
