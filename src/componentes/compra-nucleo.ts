// Reglas de "Registrar compra", sin React: se prueban con `npm test`.

/** Minúsculas y sin tildes: "Lácteos" y "lacteos" son lo mismo al buscar. */
export function normalizar(texto: string): string {
  return texto
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .trim();
}

/** Los proveedores que contienen lo escrito (sin distinguir mayúsculas ni tildes); sin texto, todos. */
export function filtrarPorNombre<T extends { nombre: string }>(lista: readonly T[], texto: string): T[] {
  const t = normalizar(texto);
  return t === "" ? [...lista] : lista.filter((p) => normalizar(p.nombre).includes(t));
}

/**
 * Si al final de la lista va "+ Agregar proveedor «texto»": cuando hay algo escrito y todavía no existe un
 * proveedor con ese mismo nombre (así no se crea dos veces el que ya está).
 */
export function ofrecerAgregar(lista: readonly { nombre: string }[], texto: string): boolean {
  const t = normalizar(texto);
  return t !== "" && !lista.some((p) => normalizar(p.nombre) === t);
}

const PARECE_CODIGO = /^[A-Za-z0-9-]+$/;

/** Lo que se escribió en el buscador de productos: ¿un código (de barras o interno) o un nombre? */
export function tipoDeBusqueda(texto: string): "codigo" | "nombre" {
  const t = texto.trim();
  // Un nombre de una palabra con letras ("Leche") también cumple el patrón; un código lleva algún dígito.
  return PARECE_CODIGO.test(t) && /\d/.test(t) ? "codigo" : "nombre";
}

/** El mensaje cuando no hay ningún producto con lo escrito, y lo que ofrece el botón "Crear producto". */
export function mensajeSinProducto(texto: string): { texto: string; codigo?: string; nombre?: string } {
  const t = texto.trim();
  return tipoDeBusqueda(t) === "codigo"
    ? { texto: `El código ${t} no está registrado.`, codigo: t }
    : { texto: `No hay ningún producto que se llame «${t}».`, nombre: t };
}
