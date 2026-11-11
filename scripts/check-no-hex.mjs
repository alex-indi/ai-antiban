#!/usr/bin/env node
// Страж токенов: в src/ui/**/*.tsx и src/features/** не должно быть hex-цветов и rgb()/rgba().
// Цвета живут только в src/styles/tokens.css. Комментарии проверяются тоже.
import { readdirSync, readFileSync, statSync, existsSync } from "node:fs";
import { join, relative, extname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(fileURLToPath(new URL(".", import.meta.url)), "..");

/** Что сканируем: [папка, допустимые расширения]. */
const targets = [
  ["src/ui", [".ts", ".tsx", ".css"]],
  ["src/features", [".ts", ".tsx", ".css"]],
];

// #rgb, #rgba, #rrggbb, #rrggbbaa — не часть слова и не якорь вида #abcxyz.
const HEX = /#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})(?![0-9a-zA-Z_-])/;
const RGB = /\brgba?\s*\(/i;

function* walk(dir) {
  for (const name of readdirSync(dir)) {
    const full = join(dir, name);
    const st = statSync(full);
    if (st.isDirectory()) yield* walk(full);
    else yield full;
  }
}

const problems = [];
let checked = 0;
for (const [dir, exts] of targets) {
  const abs = join(root, dir);
  if (!existsSync(abs)) continue; // папки может ещё не быть
  for (const file of walk(abs)) {
    if (!exts.includes(extname(file))) continue;
    checked += 1;
    readFileSync(file, "utf8")
      .split("\n")
      .forEach((line, i) => {
        const hex = line.match(HEX);
        const rgb = line.match(RGB);
        if (hex) problems.push(`${relative(root, file)}:${i + 1}: hex-цвет ${hex[0]}`);
        if (rgb) problems.push(`${relative(root, file)}:${i + 1}: ${rgb[0]}…)`);
      });
  }
}

if (problems.length > 0) {
  console.error("lint:hex — найдены «сырые» цвета, используйте токены (var(--…)):\n" + problems.join("\n"));
  process.exit(1);
}
console.log(`lint:hex — ок, проверено файлов: ${checked}`);
