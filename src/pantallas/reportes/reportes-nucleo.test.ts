// npm test: los rangos, el período en palabras y el gráfico de líneas (src/pantallas/reportes/reportes-nucleo.ts).
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  completarDias,
  diasDelRango,
  escalaY,
  indiceMasCercano,
  periodoAnterior,
  periodoEnPalabras,
  polilinea,
  puntosDeSerie,
  rangoDe,
} from "./reportes-nucleo.ts";

// Jueves 26 de noviembre de 2026.
const HOY = new Date(2026, 10, 26, 15, 30);

test("los atajos de fechas", () => {
  assert.deepEqual(rangoDe("mes", HOY), { atajo: "mes", desde: "2026-11-01", hasta: "2026-11-26" });
  assert.deepEqual(rangoDe("mesPasado", HOY), { atajo: "mesPasado", desde: "2026-10-01", hasta: "2026-10-31" });
  assert.deepEqual(rangoDe("semana", HOY), { atajo: "semana", desde: "2026-11-23", hasta: "2026-11-26" });
  assert.deepEqual(rangoDe("hoy", HOY), { atajo: "hoy", desde: "2026-11-26", hasta: "2026-11-26" });
  assert.deepEqual(rangoDe("ayer", HOY), { atajo: "ayer", desde: "2026-11-25", hasta: "2026-11-25" });
  assert.deepEqual(rangoDe("ultimos30", HOY), { atajo: "ultimos30", desde: "2026-10-28", hasta: "2026-11-26" });
  assert.deepEqual(rangoDe("anio", HOY), { atajo: "anio", desde: "2026-01-01", hasta: "2026-11-26" });
});

test("el período en palabras: el mes, un tramo del mes y un día", () => {
  assert.equal(periodoEnPalabras("2026-11-01", "2026-11-26", HOY), "Noviembre 2026");
  assert.equal(periodoEnPalabras("2026-10-01", "2026-10-31", HOY), "Octubre 2026");
  assert.equal(periodoEnPalabras("2026-11-03", "2026-11-17", HOY), "Del 3 al 17 de noviembre");
  assert.equal(periodoEnPalabras("2026-11-26", "2026-11-26", HOY), "Hoy, jueves 26 de noviembre");
  assert.equal(periodoEnPalabras("2026-11-25", "2026-11-25", HOY), "Miércoles 25 de noviembre");
});

test("el período en palabras entre meses y entre años", () => {
  assert.equal(periodoEnPalabras("2026-10-28", "2026-11-26", HOY), "Del 28 de octubre al 26 de noviembre");
  assert.equal(periodoEnPalabras("2026-01-01", "2026-11-26", HOY), "Del 1 de enero al 26 de noviembre");
  assert.equal(periodoEnPalabras("2025-12-20", "2026-01-05", HOY), "Del 20 de diciembre de 2025 al 5 de enero de 2026");
  assert.equal(periodoEnPalabras("2025-03-03", "2025-03-17", HOY), "Del 3 al 17 de marzo de 2025");
  // Un mes a medias que no llega a hoy no es "el mes".
  assert.equal(periodoEnPalabras("2026-10-01", "2026-10-15", HOY), "Del 1 al 15 de octubre");
});

test("cuántos días tiene un rango y con qué período se compara", () => {
  assert.equal(diasDelRango("2026-11-01", "2026-11-30"), 30);
  assert.equal(diasDelRango("2026-11-26", "2026-11-26"), 1);
  assert.equal(diasDelRango("2026-02-01", "2026-03-01"), 29);
  assert.deepEqual(periodoAnterior(rangoDe("mes", HOY)), { desde: "2026-10-01", hasta: "2026-10-31", nombre: "Mes pasado" });
  // Una semana que lleva 4 días se compara con los 4 días justo antes.
  assert.deepEqual(periodoAnterior(rangoDe("semana", HOY)), { desde: "2026-11-19", hasta: "2026-11-22", nombre: "Período anterior" });
  assert.equal(periodoAnterior(rangoDe("hoy", HOY)).desde, "2026-11-25");
  // Enero se compara con diciembre del año anterior.
  assert.equal(periodoAnterior({ atajo: "mes", desde: "2026-01-01", hasta: "2026-01-12" }).desde, "2025-12-01");
});

test("los días sin ventas valen 0 y no se salta ninguno", () => {
  const dias = completarDias(
    [
      { dia: "2026-11-02", monto: 5000, boletas: 2 },
      { dia: "2026-11-04", monto: 1200, boletas: 1 },
    ],
    "2026-11-01",
    "2026-11-05",
  );
  assert.deepEqual(dias.map((d) => d.dia), ["2026-11-01", "2026-11-02", "2026-11-03", "2026-11-04", "2026-11-05"]);
  assert.deepEqual(dias.map((d) => d.monto), [0, 5000, 0, 1200, 0]);
  assert.deepEqual(completarDias([], "2026-11-05", "2026-11-04"), []);
});

test("la escala del eje Y usa valores redondos", () => {
  const e = escalaY(118_000);
  assert.equal(e.valores.length, 5);
  assert.equal(e.valores[0], 0);
  assert.ok(e.tope >= 118_000 && e.tope <= 118_000 * 1.3, String(e.tope));
  assert.deepEqual(escalaY(100, 4).valores, [0, 25, 50, 75, 100]);
  assert.equal(escalaY(1_000_000).tope, 1_000_000);
});

test("la escala con todo en cero o con valores enormes", () => {
  const cero = escalaY(0);
  assert.ok(cero.tope > 0);
  assert.deepEqual(puntosDeSerie([0, 0, 0], 300, 100, cero.tope, 3).map((p) => p.y), [100, 100, 100]);
  const enorme = escalaY(9_000_000_000_000);
  assert.ok(Number.isFinite(enorme.tope) && enorme.tope >= 9_000_000_000_000);
  assert.equal(escalaY(Number.NaN).tope, cero.tope);
  assert.equal(escalaY(-5).tope, cero.tope);
});

test("los puntos: el primero a la izquierda, el último a la derecha, el máximo arriba", () => {
  const p = puntosDeSerie([0, 200, 100], 200, 100, 200, 3);
  assert.deepEqual(p.map((q) => q.x), [0, 100, 200]);
  assert.deepEqual(p.map((q) => q.y), [100, 0, 50]);
  // Una serie más corta que el eje (el mes que va) no se estira.
  assert.deepEqual(puntosDeSerie([10, 20], 300, 100, 100, 31).map((q) => Math.round(q.x)), [0, 10]);
  // Un valor por encima del tope no se sale del área.
  assert.equal(puntosDeSerie([500], 100, 100, 200, 2)[0].y, 0);
});

test("un mes con un solo día pone el punto al centro", () => {
  const p = puntosDeSerie([5000], 400, 100, 10_000, 1);
  assert.deepEqual(p, [{ x: 200, y: 50 }]);
  assert.equal(polilinea(p), "200.0,50.0");
  assert.equal(indiceMasCercano(123, 400, 1), 0);
});

test("el punto más cercano al mouse", () => {
  assert.equal(indiceMasCercano(0, 300, 31), 0);
  assert.equal(indiceMasCercano(300, 300, 31), 30);
  assert.equal(indiceMasCercano(150, 300, 31), 15);
  assert.equal(indiceMasCercano(-20, 300, 31), 0);
  assert.equal(indiceMasCercano(999, 300, 31), 30);
});
