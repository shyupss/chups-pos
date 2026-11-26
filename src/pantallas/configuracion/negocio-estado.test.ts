// npm test: Configuración > Negocio. Elegir o quitar el logo no borra lo escrito y no guardado.
import assert from "node:assert/strict";
import { test } from "node:test";
import type { Negocio } from "../../api";
import { conLogo, desdeGuardado, hayCambios } from "./negocio-estado.ts";

const enLaBase: Negocio = {
  nombre: "Almacén Doña Rosa",
  rut: null,
  direccion: "Los Aromos 123",
  telefono: null,
  mensajePie: "¡Gracias por su compra!",
  logoPng: null,
};

test("al cargar, lo escrito es lo guardado y no hay cambios", () => {
  const e = desdeGuardado(enLaBase);
  assert.equal(e.datos.rut, "");
  assert.equal(hayCambios(e.datos, e.guardados), false);
});

test("elegir el logo no toca los campos escritos ni lo guardado", () => {
  const cargado = desdeGuardado(enLaBase);
  const escrito = { ...cargado, datos: { ...cargado.datos, nombre: "Minimarket Rosa", telefono: "+56 9 1234 5678" } };
  // elegir_logo devuelve el negocio tal como está en la base (con el nombre viejo) y el logo nuevo.
  const despues = conLogo(escrito, { ...enLaBase, logoPng: "iVBORw0KGgo=" });
  assert.equal(despues.logo, "iVBORw0KGgo=");
  assert.deepEqual(despues.datos, escrito.datos);
  assert.deepEqual(despues.guardados, escrito.guardados);
  assert.equal(hayCambios(despues.datos, despues.guardados), true);
});

test("quitar el logo tampoco toca los campos", () => {
  const cargado = desdeGuardado({ ...enLaBase, logoPng: "iVBORw0KGgo=" });
  const escrito = { ...cargado, datos: { ...cargado.datos, direccion: "Av. Central 45" } };
  const despues = conLogo(escrito, { ...enLaBase, logoPng: null });
  assert.equal(despues.logo, null);
  assert.equal(despues.datos.direccion, "Av. Central 45");
});

test("hayCambios: un opcional vacío es igual a null y cualquier campo distinto cuenta", () => {
  const { datos } = desdeGuardado(enLaBase);
  assert.equal(hayCambios({ ...datos, rut: null }, datos), false);
  assert.equal(hayCambios({ ...datos, mensajePie: "Vuelva pronto" }, datos), true);
});
