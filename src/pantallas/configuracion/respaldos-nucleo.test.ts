// npm test: el estado de la última copia (src/pantallas/configuracion/respaldos-nucleo.ts).
import assert from "node:assert/strict";
import { test } from "node:test";
import { alcanceHorario, copiasAGuardar, diasDeCalendario, estadoDeLaCopia } from "./respaldos-nucleo.ts";

// Jueves 26 de noviembre de 2026, 15:30 (hora local del equipo que corre el test).
const AHORA = new Date(2026, 10, 26, 15, 30);
const iso = (a: number, m: number, d: number, h: number, min: number) => new Date(a, m - 1, d, h, min).toISOString();

test("una copia de hoy dice la hora y va en verde", () => {
  assert.deepEqual(estadoDeLaCopia(iso(2026, 11, 26, 13, 5), AHORA), { texto: "Tu última copia es de hoy a las 13:05", viejo: false });
  // Un minuto antes de la medianoche sigue siendo hoy.
  assert.equal(estadoDeLaCopia(iso(2026, 11, 26, 0, 1), AHORA).viejo, false);
});

test("ayer y hasta 3 días siguen en verde, con la fecha", () => {
  assert.deepEqual(estadoDeLaCopia(iso(2026, 11, 25, 23, 50), AHORA), { texto: "Tu última copia es de ayer a las 23:50", viejo: false });
  const tres = estadoDeLaCopia(iso(2026, 11, 23, 9, 7), AHORA);
  assert.deepEqual(tres, { texto: "Tu última copia es de hace 3 días (el 23-11-2026 a las 09:07)", viejo: false });
});

test("con más de 3 días avisa que conviene hacer una", () => {
  const vieja = estadoDeLaCopia(iso(2026, 11, 21, 10, 12), AHORA);
  assert.equal(vieja.viejo, true);
  assert.equal(vieja.texto, "Tu última copia es de hace 5 días (el 21-11-2026 a las 10:12): conviene hacer una ahora");
});

test("sin ninguna copia avisa que todavía no hay", () => {
  const r = estadoDeLaCopia(null, AHORA);
  assert.equal(r.viejo, true);
  assert.match(r.texto, /Todavía no hay copias/);
});

test("los días se cuentan por calendario, no por horas", () => {
  assert.equal(diasDeCalendario(new Date(2026, 10, 25, 23, 59), new Date(2026, 10, 26, 0, 1)), 1);
  assert.equal(diasDeCalendario(new Date(2026, 10, 26, 0, 1), new Date(2026, 10, 26, 23, 59)), 0);
  assert.equal(diasDeCalendario(new Date(2026, 9, 31, 12), new Date(2026, 10, 2, 12)), 2);
});

test("cuántas copias guardar: solo enteros de 1 a 365", () => {
  assert.equal(copiasAGuardar("5"), 5);
  assert.equal(copiasAGuardar(" 14 "), 14);
  assert.equal(copiasAGuardar("365"), 365);
  for (const malo of ["", "0", "366", "-1", "2,5", "abc", "1e2", "00000"]) assert.equal(copiasAGuardar(malo), null, malo);
});

test("con una copia por hora, el alcance son tantas horas como copias", () => {
  assert.equal(alcanceHorario(5), "las últimas 5 horas");
  assert.equal(alcanceHorario(1), "la última hora");
});
