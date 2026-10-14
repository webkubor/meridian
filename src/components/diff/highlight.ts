/**
 * Syntax colouring for editor surfaces (docs/04 §S8 "syntax colouring
 * toggle"). Our own conservative lexer: line comments, block comments across
 * lines, strings and numbers, plus a small keyword table per language family.
 *
 * It is deliberately not a parser — it can only colour what it is sure of, and
 * anything it does not recognise stays plain text. Unknown languages render
 * plain, so the toggle degrades to "off" rather than lying.
 */

export type HighlightKind = "plain" | "comment" | "string" | "keyword" | "number";

export interface HighlightToken {
  text: string;
  kind: HighlightKind;
}

interface LanguageSpec {
  lineComments: string[];
  blockComment: [string, string] | null;
  /** Only colour strings when the language has them. */
  strings: boolean;
  keywords: string[];
}

const C_LIKE_KEYWORDS = [
  "abstract", "as", "async", "await", "break", "case", "catch", "class", "const", "continue",
  "default", "delete", "do", "else", "enum", "export", "extends", "fn", "for", "from", "func",
  "function", "if", "impl", "import", "in", "instanceof", "interface", "let", "match", "mod",
  "mut", "new", "of", "package", "private", "protected", "pub", "public", "return", "self",
  "static", "struct", "super", "switch", "this", "throw", "trait", "try", "type", "typeof",
  "use", "var", "void", "while", "yield",
];

const PY_KEYWORDS = [
  "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif",
  "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda",
  "None", "nonlocal", "not", "or", "pass", "raise", "return", "True", "False", "try", "while",
  "with", "yield",
];

const SH_KEYWORDS = [
  "case", "do", "done", "elif", "else", "esac", "fi", "for", "function", "if", "in", "local",
  "return", "then", "until", "while", "export", "readonly",
];

const SPECS: Record<string, LanguageSpec> = {
  "c-like": {
    lineComments: ["//"],
    blockComment: ["/*", "*/"],
    strings: true,
    keywords: C_LIKE_KEYWORDS,
  },
  python: { lineComments: ["#"], blockComment: null, strings: true, keywords: PY_KEYWORDS },
  shell: {
    lineComments: ["#"],
    blockComment: null,
    strings: true,
    keywords: SH_KEYWORDS,
  },
  config: { lineComments: ["#", ";"], blockComment: null, strings: true, keywords: [] },
  sql: { lineComments: ["--"], blockComment: ["/*", "*/"], strings: true, keywords: [
    "select", "from", "where", "insert", "into", "values", "update", "delete", "create", "table",
    "alter", "drop", "join", "left", "right", "inner", "outer", "group", "by", "order", "limit",
    "and", "or", "not", "null", "primary", "key", "index",
  ] },
  css: { lineComments: [], blockComment: ["/*", "*/"], strings: true, keywords: [] },
  markup: { lineComments: [], blockComment: ["<!--", "-->"], strings: true, keywords: [] },
};

const EXTENSIONS: Record<string, string> = {
  js: "c-like",
  jsx: "c-like",
  mjs: "c-like",
  cjs: "c-like",
  ts: "c-like",
  tsx: "c-like",
  vue: "c-like",
  svelte: "c-like",
  java: "c-like",
  c: "c-like",
  h: "c-like",
  cpp: "c-like",
  cxx: "c-like",
  hpp: "c-like",
  cs: "c-like",
  go: "c-like",
  rs: "c-like",
  swift: "c-like",
  kt: "c-like",
  kts: "c-like",
  scala: "c-like",
  php: "c-like",
  dart: "c-like",
  json: "c-like",
  py: "python",
  rb: "python",
  sh: "shell",
  bash: "shell",
  zsh: "shell",
  fish: "shell",
  yaml: "config",
  yml: "config",
  toml: "config",
  ini: "config",
  cfg: "config",
  conf: "config",
  properties: "config",
  env: "config",
  sql: "sql",
  css: "css",
  scss: "css",
  less: "css",
  html: "markup",
  htm: "markup",
  xml: "markup",
  svg: "markup",
  md: "markup",
  markdown: "markup",
};

export function languageForPath(path: string | null): string | null {
  if (!path) return null;
  const name = path.split("/").pop() ?? path;
  const dot = name.lastIndexOf(".");
  if (dot <= 0) {
    if (name === "Makefile" || name === "Dockerfile") return "shell";
    return null;
  }
  return EXTENSIONS[name.slice(dot + 1).toLowerCase()] ?? null;
}

const IDENT_START = /[A-Za-z_$]/;
const IDENT_PART = /[A-Za-z0-9_$]/;
const DIGIT = /[0-9]/;
const HEX_DIGIT = /[0-9a-fA-F]/;

function isKeyword(spec: LanguageSpec, word: string): boolean {
  if (spec.keywords.length === 0) return false;
  return spec.keywords.includes(word) || spec.keywords.includes(word.toLowerCase());
}

/**
 * A stateful scanner: block comments can span lines, so one instance colours
 * one document in order.
 */
export function createHighlighter(language: string | null): (text: string) => HighlightToken[] {
  const spec = language ? SPECS[language] ?? null : null;
  let inBlockComment = false;

  if (!spec) {
    return (text) => (text.length > 0 ? [{ text, kind: "plain" }] : []);
  }

  return (text: string): HighlightToken[] => {
    const tokens: HighlightToken[] = [];
    let plain = "";

    const flushPlain = (): void => {
      if (plain.length > 0) {
        tokens.push({ text: plain, kind: "plain" });
        plain = "";
      }
    };

    let index = 0;
    while (index < text.length) {
      if (inBlockComment && spec.blockComment) {
        const close = spec.blockComment[1];
        const end = text.indexOf(close, index);
        if (end === -1) {
          tokens.push({ text: text.slice(index), kind: "comment" });
          index = text.length;
        } else {
          tokens.push({ text: text.slice(index, end + close.length), kind: "comment" });
          index = end + close.length;
          inBlockComment = false;
        }
        continue;
      }

      const lineComment = spec.lineComments.find((marker) => text.startsWith(marker, index));
      if (lineComment) {
        flushPlain();
        tokens.push({ text: text.slice(index), kind: "comment" });
        return tokens;
      }

      if (spec.blockComment && text.startsWith(spec.blockComment[0], index)) {
        flushPlain();
        const [open, close] = spec.blockComment;
        const end = text.indexOf(close, index + open.length);
        if (end === -1) {
          tokens.push({ text: text.slice(index), kind: "comment" });
          inBlockComment = true;
          index = text.length;
        } else {
          tokens.push({ text: text.slice(index, end + close.length), kind: "comment" });
          index = end + close.length;
        }
        continue;
      }

      const char = text[index];
      if (spec.strings && (char === '"' || char === "'" || char === "`")) {
        flushPlain();
        let cursor = index + 1;
        while (cursor < text.length) {
          if (text[cursor] === "\\") {
            cursor += 2;
            continue;
          }
          if (text[cursor] === char) {
            cursor += 1;
            break;
          }
          cursor += 1;
        }
        tokens.push({ text: text.slice(index, cursor), kind: "string" });
        index = cursor;
        continue;
      }

      if (DIGIT.test(char)) {
        flushPlain();
        let cursor = index;
        const hex = text.slice(index, index + 2).toLowerCase() === "0x";
        while (cursor < text.length) {
          const c = text[cursor];
          if (hex
            ? HEX_DIGIT.test(c)
            : DIGIT.test(c) || c === "." || c === "_" || c === "e" || c === "E") {
            cursor += 1;
          } else {
            break;
          }
        }
        tokens.push({ text: text.slice(index, cursor), kind: "number" });
        index = cursor;
        continue;
      }

      if (IDENT_START.test(char)) {
        let cursor = index + 1;
        while (cursor < text.length && IDENT_PART.test(text[cursor])) cursor += 1;
        const word = text.slice(index, cursor);
        if (isKeyword(spec, word)) {
          flushPlain();
          tokens.push({ text: word, kind: "keyword" });
        } else {
          plain += word;
        }
        index = cursor;
        continue;
      }

      plain += char;
      index += 1;
    }

    flushPlain();
    return tokens;
  };
}
