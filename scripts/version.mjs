// Una sola fuente para la versión: el "version" de package.json.
//
//   node scripts/version.mjs            sincroniza tauri.conf.json, Cargo.toml (workspace) y package-lock.json
//   node scripts/version.mjs --check    falla si alguno no coincide (lo usa la CI)
//   node scripts/version.mjs --check --tag v1.0.0-rc.1
//                                       además exige que la etiqueta sea la versión (con o sin sufijo -rc.N)
//   node scripts/version.mjs --set 1.1.0   cambia la versión en package.json y sincroniza todo
//
// Cargo.lock se actualiza solo con el siguiente `cargo build` (la CI usa --locked, así que hay que
// commitearlo después de cambiar la versión).
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const raiz = join(dirname(fileURLToPath(import.meta.url)), "..");
const ruta = (r) => join(raiz, r);
const leer = (r) => readFileSync(ruta(r), "utf8");
const args = process.argv.slice(2);
const opcion = (n) => (args.includes(n) ? args[args.indexOf(n) + 1] : null);

const SEMVER = /^\d+\.\d+\.\d+$/;
const eol = (t) => (t.includes("\r\n") ? "\r\n" : "\n");

function escribir(r, texto, original) {
  if (texto !== original) writeFileSync(ruta(r), texto);
  return texto !== original;
}

/** Cada lugar donde vive la versión: cómo leerla y cómo escribirla. */
const lugares = [
  {
    archivo: "src-tauri/tauri.conf.json",
    leer: () => JSON.parse(leer("src-tauri/tauri.conf.json")).version,
    escribir: (v) => {
      const r = "src-tauri/tauri.conf.json";
      const t = leer(r);
      return escribir(r, t.replace(/("version"\s*:\s*")[^"]*(")/, `$1${v}$2`), t);
    },
  },
  {
    archivo: "Cargo.toml ([workspace.package])",
    leer: () => leer("Cargo.toml").match(/\[workspace\.package\][^[]*?\bversion\s*=\s*"([^"]+)"/)?.[1],
    escribir: (v) => {
      const t = leer("Cargo.toml");
      return escribir("Cargo.toml", t.replace(/(\[workspace\.package\][^[]*?\bversion\s*=\s*")[^"]+(")/, `$1${v}$2`), t);
    },
  },
  {
    archivo: "package-lock.json",
    leer: () => {
      const j = JSON.parse(leer("package-lock.json"));
      return j.version === j.packages?.[""]?.version ? j.version : `${j.version} / ${j.packages?.[""]?.version}`;
    },
    escribir: (v) => {
      const t = leer("package-lock.json");
      const j = JSON.parse(t);
      j.version = v;
      if (j.packages?.[""]) j.packages[""].version = v;
      return escribir("package-lock.json", JSON.stringify(j, null, 2).replace(/\n/g, eol(t)) + eol(t), t);
    },
  },
];

const paquete = JSON.parse(leer("package.json"));
let fuente = paquete.version;

const nueva = opcion("--set");
if (nueva) {
  if (!SEMVER.test(nueva)) {
    console.error(`La versión debe ser X.Y.Z (sin sufijos): ${nueva}`);
    process.exit(2);
  }
  const t = leer("package.json");
  writeFileSync(ruta("package.json"), t.replace(/("version"\s*:\s*")[^"]*(")/, `$1${nueva}$2`));
  fuente = nueva;
}
if (!SEMVER.test(fuente)) {
  console.error(`package.json tiene una versión que no es X.Y.Z: ${fuente}`);
  process.exit(2);
}

if (args.includes("--check")) {
  let mal = 0;
  console.log(`Versión (package.json): ${fuente}`);
  for (const l of lugares) {
    const v = l.leer();
    const ok = v === fuente;
    if (!ok) mal++;
    console.log(`  ${ok ? "ok " : "MAL"} ${l.archivo}: ${v}`);
  }
  const tag = opcion("--tag");
  if (tag) {
    // v1.0.0 o v1.0.0-rc.1: el núcleo X.Y.Z tiene que ser la versión de los archivos.
    const nucleo = tag.replace(/^v/, "").split("-")[0];
    const ok = nucleo === fuente;
    if (!ok) mal++;
    console.log(`  ${ok ? "ok " : "MAL"} etiqueta ${tag}: ${nucleo}`);
  }
  if (mal) {
    console.error(`\n${mal} lugar(es) con otra versión. Corrige con: node scripts/version.mjs`);
    process.exit(1);
  }
  process.exit(0);
}

let cambios = 0;
for (const l of lugares) {
  if (l.leer() !== fuente) {
    l.escribir(fuente);
    cambios++;
    console.log(`  actualizado ${l.archivo} -> ${fuente}`);
  }
}
console.log(cambios ? `Versión ${fuente} sincronizada.` : `Todo en ${fuente}: nada que cambiar.`);
