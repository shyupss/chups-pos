import { useCallback, useEffect, useRef, useState } from "react";
import { api, textoError, type Config } from "../api";

/**
 * Configuración de la tabla config para la pantalla de Configuración.
 * `cambiar` muestra el cambio al instante y lo guarda; si Rust lo rechaza, vuelve atrás y deja el error.
 */
export function useConfig() {
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

  const cambiar = useCallback(async (parcial: Partial<Config>) => {
    const anterior = ultima.current;
    if (!anterior) return;
    const nueva = { ...anterior, ...parcial };
    ultima.current = nueva;
    setConfig(nueva);
    setError(null);
    try {
      const guardada = await api.guardarConfig(nueva);
      ultima.current = guardada;
      setConfig(guardada);
    } catch (e) {
      ultima.current = anterior;
      setConfig(anterior);
      setError(textoError(e));
    }
  }, []);

  return { config, error, cambiar };
}

export type ConfigEditable = ReturnType<typeof useConfig>;
