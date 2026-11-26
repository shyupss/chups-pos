// npm test: las reglas del despachador de atajos (src/atajos-nucleo.ts).
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  combinacion,
  esDeCaptura,
  esTeclaDelNavegador,
  IMPRIMIBLE,
  resolver,
  type Registro,
  type Tecla,
} from "./atajos-nucleo.ts";

const tecla = (key: string, mods: Partial<Omit<Tecla, "key">> = {}): Tecla => ({
  key,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

const SECCIONES: Registro = { ambito: "global", orden: 1, teclas: ["F1", "F2", "F3", "F4", "F5", "F6", "F9"] };
const CAJA: Registro = { ambito: "pantalla", orden: 2, teclas: ["F12", "Ctrl+P", "F7", "F8", "Escape", IMPRIMIBLE] };
const COBRO: Registro = { ambito: "dialogo", orden: 10, teclas: ["Escape", "Enter", "F12", "1", "Alt+1"] };
const PIN: Registro = { ambito: "dialogo", orden: 11, teclas: ["Escape"] };

test("combinacion: F, Ctrl, Alt, letras en mayúscula y sin Shift en un carácter suelto", () => {
  assert.equal(combinacion(tecla("F1")), "F1");
  assert.equal(combinacion(tecla("p", { ctrlKey: true })), "Ctrl+P");
  assert.equal(combinacion(tecla("P", { ctrlKey: true, shiftKey: true })), "Ctrl+P");
  assert.equal(combinacion(tecla("1", { altKey: true })), "Alt+1");
  assert.equal(combinacion(tecla("F5", { shiftKey: true })), "Shift+F5");
  assert.equal(combinacion(tecla("+", { ctrlKey: true, shiftKey: true })), "Ctrl++");
  assert.equal(combinacion(tecla("Escape")), "Escape");
  assert.equal(combinacion(tecla("a")), "A");
});

test("sin diálogo: los atajos de sección y de pantalla funcionan", () => {
  assert.equal(resolver([SECCIONES, CAJA], "F2", false)?.registro, SECCIONES);
  assert.equal(resolver([SECCIONES, CAJA], "F12", false)?.registro, CAJA);
});

test("un diálogo abierto bloquea los atajos de sección y de pantalla", () => {
  for (const f of ["F1", "F2", "F3", "F4", "F5", "F6", "F9"]) {
    assert.equal(resolver([SECCIONES, CAJA, COBRO], f, false), null, f);
  }
  assert.equal(resolver([SECCIONES, CAJA, COBRO], "F7", false), null);
  assert.equal(resolver([SECCIONES, CAJA, COBRO], "Ctrl+P", false), null);
});

test("Esc va al diálogo de más arriba", () => {
  assert.equal(resolver([SECCIONES, CAJA, COBRO], "Escape", false)?.registro, COBRO);
  assert.equal(resolver([SECCIONES, CAJA, COBRO, PIN], "Escape", false)?.registro, PIN);
  // Con el PIN encima, las teclas del cobro no corren.
  assert.equal(resolver([SECCIONES, CAJA, COBRO, PIN], "F12", false), null);
  // El orden de la lista no importa: manda el orden de aparición.
  assert.equal(resolver([PIN, COBRO, CAJA], "Escape", false)?.registro, PIN);
});

test("las F y Ctrl funcionan con el foco en un campo; las letras sueltas no", () => {
  assert.equal(resolver([SECCIONES, CAJA], "F2", true)?.registro, SECCIONES);
  assert.equal(resolver([SECCIONES, CAJA], "Ctrl+P", true)?.registro, CAJA);
  assert.equal(resolver([SECCIONES, CAJA, COBRO], "Alt+1", true)?.registro, COBRO);
  assert.equal(resolver([SECCIONES, CAJA, COBRO], "1", true), null);
  assert.equal(resolver([SECCIONES, CAJA, COBRO], "1", false)?.registro, COBRO);
  assert.equal(resolver([SECCIONES, CAJA], "A", true), null);
});

test("el foco pegajoso: un carácter suelto fuera de un campo va al comodín de la pantalla", () => {
  assert.deepEqual(resolver([SECCIONES, CAJA], "7", false), { registro: CAJA, clave: IMPRIMIBLE });
  assert.equal(resolver([SECCIONES, CAJA], "7", true), null);
  // Con un diálogo abierto, el comodín de la pantalla no corre.
  assert.equal(resolver([SECCIONES, CAJA, PIN], "7", false), null);
});

test("en la misma pantalla gana lo que apareció después (el detalle sobre la lista)", () => {
  const ventas: Registro = { ambito: "pantalla", orden: 3, teclas: [IMPRIMIBLE] };
  const detalle: Registro = { ambito: "pantalla", orden: 4, teclas: ["Ctrl+P", "Escape"] };
  assert.equal(resolver([ventas, detalle], "Escape", false)?.registro, detalle);
  assert.equal(resolver([ventas, detalle], "5", false)?.registro, ventas);
});

test("las teclas del navegador llevan preventDefault aunque nadie las atienda", () => {
  for (const c of ["F3", "F5", "Shift+F5", "Ctrl+F5", "F7", "F12", "Ctrl+F", "Ctrl+R", "Ctrl+S", "Ctrl+P", "Ctrl++", "Ctrl+-", "Ctrl+0"]) {
    assert.ok(esTeclaDelNavegador(c), c);
  }
  // Copiar, pegar, cortar, deshacer y seleccionar todo no se tocan; Alt+F4 cierra la ventana.
  for (const c of ["Ctrl+C", "Ctrl+V", "Ctrl+X", "Ctrl+Z", "Ctrl+A", "Alt+F4", "Escape", "A"]) {
    assert.ok(!esTeclaDelNavegador(c), c);
  }
});

test("fase: F, Ctrl y Alt se atienden en captura; Esc, Enter y caracteres en burbuja", () => {
  for (const c of ["F1", "F12", "Shift+F5", "Ctrl+P", "Alt+1", "Ctrl+Shift+R"]) assert.ok(esDeCaptura(c), c);
  for (const c of ["Escape", "Enter", "1", "A", "ArrowDown"]) assert.ok(!esDeCaptura(c), c);
});
