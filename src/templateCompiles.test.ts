/**
 * Every single-file component must compile.
 *
 * `vue-tsc` is lenient about malformed templates: an element that is opened
 * and never closed typechecks cleanly, and only fails when Vite compiles the
 * SFC for a production build. That is exactly how an unclosed `<ComparePane>`
 * sat green through typecheck and the whole suite and then broke `tauri build`.
 *
 * Compiling every template here moves that failure to `pnpm test`, where it
 * costs a second instead of a release attempt.
 */

// @vitest-environment node

import { readFileSync, readdirSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

import { compileTemplate, parse } from "@vue/compiler-sfc";
import { describe, expect, it } from "vitest";

const SRC = fileURLToPath(new URL(".", import.meta.url));

function vueFiles(dir = SRC): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...vueFiles(path));
    else if (entry.name.endsWith(".vue")) out.push(path);
  }
  return out;
}

describe("single-file components compile", () => {
  const files = vueFiles();

  it("finds the components, so the loop below cannot pass vacuously", () => {
    expect(files.length).toBeGreaterThan(40);
  });

  it.each(files.map((path) => [relative(SRC, path), path] as const))("%s", (name, path) => {
    const source = readFileSync(path, "utf8");
    const { descriptor, errors: parseErrors } = parse(source, { filename: name });
    expect(parseErrors.map((error) => error.message)).toEqual([]);

    if (descriptor.template) {
      const result = compileTemplate({
        source: descriptor.template.content,
        filename: name,
        id: name,
      });
      expect(result.errors.map((error) => (typeof error === "string" ? error : error.message))).toEqual([]);
    }
  });
});
