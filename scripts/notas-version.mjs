// Saca de CHANGELOG.md las notas de una versión (el apartado "## [X.Y.Z] - fecha").
//
//   node scripts/notas-version.mjs 1.0.0          imprime las notas
//   node scripts/notas-version.mjs v1.0.0-rc.1    acepta la etiqueta: usa el núcleo X.Y.Z
//
// Falla (código 1) si la versión no tiene apartado: así un Release no sale sin notas.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const raiz = join(dirname(fileURLToPath(import.meta.url)), "..");
const pedida = (process.argv[2] ?? "").replace(/^v/, "").split("-")[0];
if (!/^\d+\.\d+\.\d+$/.test(pedida)) {
  console.error("Uso: node scripts/notas-version.mjs X.Y.Z");
  process.exit(2);
}

const lineas = readFileSync(join(raiz, "CHANGELOG.md"), "utf8").split(/\r?\n/);
const encabezado = (l) => l.match(/^##\s+\[?v?(\d+\.\d+\.\d+)\]?/);
const inicio = lineas.findIndex((l) => encabezado(l)?.[1] === pedida);
if (inicio < 0) {
  console.error(`CHANGELOG.md no tiene un apartado para la versión ${pedida}.`);
  process.exit(1);
}
let fin = lineas.findIndex((l, i) => i > inicio && /^##\s/.test(l));
if (fin < 0) fin = lineas.length;
const notas = lineas
  .slice(inicio + 1, fin)
  .join("\n")
  .trim();
if (!notas) {
  console.error(`El apartado de ${pedida} está vacío.`);
  process.exit(1);
}
console.log(notas);
