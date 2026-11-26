// npm test: las reglas de Registrar compra (src/componentes/compra-nucleo.ts).
import assert from "node:assert/strict";
import { test } from "node:test";
import { filtrarPorNombre, mensajeSinProducto, normalizar, ofrecerAgregar, tipoDeBusqueda } from "./compra-nucleo.ts";

const proveedores = [{ nombre: "Distribuidora Sur" }, { nombre: "Lácteos del Valle" }, { nombre: "Don Pepe" }];

test("busca proveedores sin distinguir mayúsculas ni tildes", () => {
  assert.deepEqual(filtrarPorNombre(proveedores, "lacteos").map((p) => p.nombre), ["Lácteos del Valle"]);
  assert.deepEqual(filtrarPorNombre(proveedores, "  DISTRI ").map((p) => p.nombre), ["Distribuidora Sur"]);
  assert.equal(filtrarPorNombre(proveedores, "").length, 3);
  assert.equal(filtrarPorNombre(proveedores, "zzz").length, 0);
});

test("ofrece agregar un proveedor solo si hay texto y no existe uno con ese nombre", () => {
  assert.equal(ofrecerAgregar(proveedores, ""), false);
  assert.equal(ofrecerAgregar(proveedores, "   "), false);
  assert.equal(ofrecerAgregar(proveedores, "Panadería Rosa"), true);
  assert.equal(ofrecerAgregar(proveedores, "don pepe"), false);
  assert.equal(ofrecerAgregar(proveedores, "lacteos del valle"), false);
  // Una parte del nombre no es el mismo nombre: se puede crear "Don".
  assert.equal(ofrecerAgregar(proveedores, "Don"), true);
});

test("normalizar quita tildes y espacios de los bordes", () => {
  assert.equal(normalizar("  Árbol Ñandú "), "arbol nandu");
});

test("distingue un código de un nombre", () => {
  assert.equal(tipoDeBusqueda("7801234567890"), "codigo");
  assert.equal(tipoDeBusqueda("AB-123"), "codigo");
  assert.equal(tipoDeBusqueda("Leche"), "nombre");
  assert.equal(tipoDeBusqueda("Leche entera 1 L"), "nombre");
  assert.equal(tipoDeBusqueda("leche1"), "codigo");
});

test("el mensaje de producto inexistente ofrece crearlo con el código o con el nombre", () => {
  assert.deepEqual(mensajeSinProducto(" 7801234567890 "), { texto: "El código 7801234567890 no está registrado.", codigo: "7801234567890" });
  assert.deepEqual(mensajeSinProducto("Galletas de agua"), { texto: "No hay ningún producto que se llame «Galletas de agua».", nombre: "Galletas de agua" });
});
