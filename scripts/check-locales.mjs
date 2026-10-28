// Checks every src/locales/*.ts against en.ts: same keys, same {placeholders}.
// Usage: npm run check-locales

import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const dir = join(dirname(fileURLToPath(import.meta.url)), "../src/locales");

/** The locale files are a plain object literal after `export default`. */
function load(file) {
  const src = readFileSync(join(dir, file), "utf8");
  const body = src.slice(src.indexOf("export default") + "export default".length).trim().replace(/;\s*$/, "");
  return Function(`"use strict"; return (${body});`)();
}

const holes = (s) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(",");
const en = load("en.ts");
let problems = 0;
for (const file of readdirSync(dir).filter((f) => f.endsWith(".ts") && f !== "en.ts")) {
  const dict = load(file);
  const report = (msg) => {
    problems++;
    console.log(`${file}: ${msg}`);
  };
  for (const key of Object.keys(en)) {
    if (!(key in dict)) report(`missing "${key}"`);
    else if (holes(dict[key]) !== holes(en[key])) report(`"${key}" placeholders {${holes(dict[key])}} ≠ {${holes(en[key])}}`);
  }
  for (const key of Object.keys(dict)) if (!(key in en)) report(`unknown "${key}"`);
}
console.log(problems ? `${problems} problem(s)` : "all locales match en.ts");
process.exit(problems ? 1 : 0);
