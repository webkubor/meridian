/**
 * Catalogue parity — the runtime proof behind `zh-CN.ts`'s `Catalog` type.
 *
 * `vue-tsc` already refuses a build where one catalogue has a key the other
 * lacks (`zh-CN.ts` is annotated `Catalog`, the shape of `en.ts`). Two things
 * the type system cannot see, and this file can:
 *
 *  1. a key whose Chinese value is a copy of the English one — `Catalog` is a
 *     plain `string` shape, so "Cancel" satisfies it exactly as well as "取消";
 *  2. drift between the *runtime* objects, independent of the declared types
 *     (a spread, a computed key or a `satisfies`-only annotation can produce a
 *     key that no `vue-tsc` error would ever mention).
 */

import { describe, expect, it } from "vitest";

import en from "./locales/en";
import zhCN from "./locales/zh-CN";

/** Every leaf as `dotted.path -> value`; the catalogues are strings all the way down. */
function flatten(tree: unknown, prefix = ""): Map<string, string> {
  const leaves = new Map<string, string>();
  if (tree !== null && typeof tree === "object") {
    for (const [key, child] of Object.entries(tree)) {
      const path = prefix === "" ? key : `${prefix}.${key}`;
      for (const [leafPath, value] of flatten(child, path)) {
        leaves.set(leafPath, value);
      }
    }
  } else {
    leaves.set(prefix, String(tree));
  }
  return leaves;
}

const enLeaves = flatten(en);
const zhLeaves = flatten(zhCN);
const enPaths = [...enLeaves.keys()].sort();
const zhPaths = [...zhLeaves.keys()].sort();

/**
 * Paths where the Chinese value is allowed to equal the English one byte for
 * byte. Each entry is a claim that the string is genuinely untranslatable —
 * keep the list this short. Anything else flagged below is an untranslated
 * string to fix in `zh-CN.ts`, not a line to add here.
 */
const UNTRANSLATED_BY_DESIGN = new Set([
  // A product name. en.ts states the convention outright: a product name is
  // not a translatable string.
  "app.name",
  // The language names say themselves: the English UI must offer "简体中文",
  // and the Chinese UI must offer "English" — labelling the English language
  // "英语" would show the choice in a script the chooser may not read.
  "language.en",
  "language.zhCN",
]);

describe("i18n catalogues", () => {
  it("walks real leaves, so the parity checks cannot pass vacuously", () => {
    // A walker that quietly returned nothing would make every assertion below
    // succeed on two empty sets; pin a deep path and the overall size.
    expect(enPaths).toContain("errors.byCode.unclassified");
    expect(enPaths).toContain("updates.disclosureTitle");
    expect(zhPaths).toContain("errors.byCode.index_locked");
    expect(enPaths.length).toBeGreaterThan(50);
  });

  it("has the same key paths in en and zh-CN", () => {
    const missingFromChinese = enPaths.filter((path) => !zhLeaves.has(path));
    const missingFromEnglish = zhPaths.filter((path) => !enLeaves.has(path));
    expect(missingFromChinese, "keys in en.ts with no zh-CN.ts counterpart").toEqual([]);
    expect(missingFromEnglish, "keys in zh-CN.ts with no en.ts counterpart").toEqual([]);
    expect(zhPaths).toEqual(enPaths);
  });

  it("has no Chinese value that is a copy of its English counterpart", () => {
    const copied = [...zhLeaves]
      .filter(([path, value]) => enLeaves.get(path) === value && !UNTRANSLATED_BY_DESIGN.has(path))
      .map(([path]) => path);
    expect(
      copied,
      "value is byte-identical to en.ts — translate it in zh-CN.ts, or allowlist the path in UNTRANSLATED_BY_DESIGN with a reason",
    ).toEqual([]);
  });

  it("has no allowlisted path that stopped being a duplicate", () => {
    // An allowlist entry that no longer describes reality is stale permission:
    // once the strapline is translated, its line here must go too.
    const stale = [...UNTRANSLATED_BY_DESIGN].filter(
      (path) => enLeaves.get(path) !== zhLeaves.get(path),
    );
    expect(stale, "allowlisted path is no longer identical in both catalogues — remove it").toEqual(
      [],
    );
  });
});
