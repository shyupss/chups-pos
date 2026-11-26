// npm test: los valores por defecto de una instalación nueva (src/preferencias-nucleo.ts).
import assert from "node:assert/strict";
import { test } from "node:test";
import { defectosParaInstalacionNueva } from "./preferencias-nucleo.ts";

test("una instalación nueva sin nada guardado arranca en oscuro y con la barra expandida", () => {
  assert.deepEqual(defectosParaInstalacionNueva({ instalacionNueva: true, hayTema: false, hayBarra: false }), {
    tema: "oscuro",
    barra: "expandida",
  });
});

test("lo que ya estaba guardado manda, aunque la instalación sea nueva", () => {
  assert.deepEqual(defectosParaInstalacionNueva({ instalacionNueva: true, hayTema: true, hayBarra: false }), {
    barra: "expandida",
  });
  assert.deepEqual(defectosParaInstalacionNueva({ instalacionNueva: true, hayTema: false, hayBarra: true }), {
    tema: "oscuro",
  });
  assert.deepEqual(defectosParaInstalacionNueva({ instalacionNueva: true, hayTema: true, hayBarra: true }), {});
});

test("un equipo que ya venía usando la app no cambia, aunque nunca haya elegido nada", () => {
  assert.deepEqual(defectosParaInstalacionNueva({ instalacionNueva: false, hayTema: false, hayBarra: false }), {});
  assert.deepEqual(defectosParaInstalacionNueva({ instalacionNueva: false, hayTema: true, hayBarra: true }), {});
});
