// npm test: las reglas de la tabla común (src/componentes/tabla-nucleo.ts).
import assert from "node:assert/strict";
import { test } from "node:test";
import { columnasVisibles, ordenarFilas, plantillaGrid, siguienteOrden, textoConteo } from "./tabla-nucleo.ts";

test("el clic en un encabezado va de ascendente a descendente y a sin orden", () => {
  const a = siguienteOrden(null, "precio");
  assert.deepEqual(a, { columna: "precio", direccion: "asc" });
  const b = siguienteOrden(a, "precio");
  assert.deepEqual(b, { columna: "precio", direccion: "desc" });
  assert.equal(siguienteOrden(b, "precio"), null);
  // Otra columna empieza de nuevo en ascendente.
  assert.deepEqual(siguienteOrden(b, "nombre"), { columna: "nombre", direccion: "asc" });
});

const filas = [
  { n: "pan", p: 1000, id: 1 },
  { n: "Arroz", p: 2500, id: 2 },
  { n: "álamo", p: null, id: 3 },
  { n: "leche", p: 990, id: 4 },
  { n: "azúcar", p: 990, id: 5 },
];

test("ordena números como números y texto sin distinguir mayúsculas ni tildes", () => {
  assert.deepEqual(ordenarFilas(filas, (f) => f.p, "asc").map((f) => f.id), [4, 5, 1, 2, 3]);
  assert.deepEqual(ordenarFilas(filas, (f) => f.p, "desc").map((f) => f.id), [2, 1, 4, 5, 3]);
  assert.deepEqual(ordenarFilas(filas, (f) => f.n, "asc").map((f) => f.n), ["álamo", "Arroz", "azúcar", "leche", "pan"]);
});

test("lo que no tiene valor va siempre al final y el orden es estable", () => {
  assert.equal(ordenarFilas(filas, (f) => f.p, "asc").at(-1)?.id, 3);
  assert.equal(ordenarFilas(filas, (f) => f.p, "desc").at(-1)?.id, 3);
  // 4 y 5 valen lo mismo: se quedan en el orden en que estaban, en los dos sentidos.
  const asc = ordenarFilas(filas, (f) => f.p, "asc").map((f) => f.id);
  const desc = ordenarFilas(filas, (f) => f.p, "desc").map((f) => f.id);
  assert.ok(asc.indexOf(4) < asc.indexOf(5));
  assert.ok(desc.indexOf(4) < desc.indexOf(5));
});

test("no modifica la lista original", () => {
  const copia = filas.map((f) => f.id);
  ordenarFilas(filas, (f) => f.n, "desc");
  assert.deepEqual(filas.map((f) => f.id), copia);
});

test("los números dentro del texto se ordenan como números (folio 2 antes que folio 10)", () => {
  const f = [{ x: "Folio 10" }, { x: "Folio 2" }, { x: "Folio 1" }];
  assert.deepEqual(ordenarFilas(f, (r) => r.x, "asc").map((r) => r.x), ["Folio 1", "Folio 2", "Folio 10"]);
});

test("las columnas que no caben se ocultan; sin medir se ven todas", () => {
  const cols = [{ id: "a" }, { id: "b", ocultarBajo: 500 }, { id: "c", ocultarBajo: 800 }];
  assert.deepEqual(columnasVisibles(cols, 0).map((c) => c.id), ["a", "b", "c"]);
  assert.deepEqual(columnasVisibles(cols, 400).map((c) => c.id), ["a"]);
  assert.deepEqual(columnasVisibles(cols, 500).map((c) => c.id), ["a", "b"]);
  assert.deepEqual(columnasVisibles(cols, 1200).map((c) => c.id), ["a", "b", "c"]);
});

test("la plantilla del grid deja una pista libre para la flecha solo si la fila se abre", () => {
  assert.equal(plantillaGrid(["10rem", "6rem"], false), "10rem 6rem");
  assert.equal(plantillaGrid(["10rem", "6rem"], true), "10rem 6rem minmax(1.5rem,auto)");
});

test("el conteo usa singular, plural y separador de miles", () => {
  assert.equal(textoConteo(1, "producto", "productos"), "1 producto");
  assert.equal(textoConteo(0, "producto", "productos"), "0 productos");
  assert.equal(textoConteo(1234, "producto", "productos"), "1.234 productos");
  assert.equal(textoConteo(100, "producto", "productos", true), "100 productos (baja para ver más)");
});
