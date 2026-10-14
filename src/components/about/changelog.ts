/**
 * A reader for the Keep a Changelog files the app ships.
 *
 * The changelog is Markdown, but this deliberately is not a Markdown renderer.
 * Two reasons:
 *
 * 1. **The notes are rendered with `v-html`-free Vue templates.** A general
 *    Markdown renderer produces HTML, and handing that to `v-html` would mean
 *    trusting the strings — including the release body that arrives over the
 *    network in the update manifest. Parsing to a data structure and rendering
 *    it with elements means there is no HTML sink in the app at all, so a
 *    hostile release note is a rendering curiosity rather than a script
 *    execution.
 * 2. **The format is ours and narrow.** `changelog_parity.rs` already requires a
 *    specific shape (`## [version] - date`, `### Group`, `- item`), so a
 *    parser for exactly that is smaller than configuring a general one, and a
 *    malformed file is caught by a unit test rather than rendering as raw
 *    asterisks.
 *
 * Anything outside that shape is not discarded: unrecognised lines accumulate
 * into the *current* block as prose, so a future edit to the changelog degrades
 * into readable text instead of vanishing.
 */

export interface ChangelogEntry {
  /** The bracketed label verbatim: `0.1.0`, or `Unreleased`. */
  version: string;
  /** True for the `[Unreleased]` section, which has no date. */
  unreleased: boolean;
  /** `YYYY-MM-DD` as written, or null when the heading carried no date. */
  date: string | null;
  /** Prose between the heading and the first `###`, one string per paragraph. */
  summary: string[];
  groups: ChangelogGroup[];
}

export interface ChangelogGroup {
  title: string;
  items: ChangelogItem[];
}

export interface ChangelogItem {
  text: string;
  /** 0 for a top-level bullet, 1 for a nested one. */
  depth: number;
}

export interface Changelog {
  /** Prose above the first `##`, one string per paragraph. */
  preamble: string[];
  entries: ChangelogEntry[];
}

/** `## [0.1.0] - 2026-10-14`, `## [Unreleased]`, `## [0.2.0]`. */
const VERSION_HEADING = /^##\s+\[([^\]]+)\]\s*(?:-\s*(\S+))?\s*$/;
/** `### Added`. */
const GROUP_HEADING = /^###\s+(.+?)\s*$/;
/** `- item`, `* item`. */
const BULLET = /^(\s*)[-*]\s+(.*)$/;
/** `[label]: https://…` — link definitions, which carry no reader-facing text. */
const LINK_DEFINITION = /^\s*\[[^\]]+\]:\s*\S+\s*$/;

/**
 * Parses a Keep a Changelog document.
 *
 * Never throws: a file that does not match the expected shape yields whatever
 * was understood. The changelog is compiled into the binary, so a parse failure
 * is a build-time authoring mistake, and the app's job is still to show the
 * reader the release history rather than an error screen.
 */
export function parseChangelog(markdown: string): Changelog {
  const lines = markdown.split(/\r?\n/);

  const preamble: string[] = [];
  const entries: ChangelogEntry[] = [];
  let entry: ChangelogEntry | null = null;
  let group: ChangelogGroup | null = null;

  /** The list or paragraph the next continuation line belongs to. */
  let openItem: ChangelogItem | null = null;
  let paragraph: { target: string[]; index: number } | null = null;

  const closeParagraph = () => {
    paragraph = null;
  };
  const closeItem = () => {
    openItem = null;
  };
  const closeBlocks = () => {
    closeItem();
    closeParagraph();
  };

  for (const raw of lines) {
    const line = raw.trimEnd();

    if (LINK_DEFINITION.test(line)) {
      closeBlocks();
      continue;
    }

    const versionMatch = VERSION_HEADING.exec(line);
    if (versionMatch) {
      closeBlocks();
      const [, label, date] = versionMatch;
      entry = {
        version: label,
        unreleased: label.toLowerCase() === "unreleased",
        date: date ?? null,
        summary: [],
        groups: [],
      };
      entries.push(entry);
      group = null;
      continue;
    }

    const groupMatch = GROUP_HEADING.exec(line);
    if (groupMatch && entry) {
      closeBlocks();
      group = { title: groupMatch[1], items: [] };
      entry.groups.push(group);
      continue;
    }

    const bulletMatch = BULLET.exec(line);
    if (bulletMatch && entry) {
      closeBlocks();
      const [, indent, text] = bulletMatch;
      // Two spaces per level is what the file uses; anything deeper is clamped
      // rather than producing an unbounded indent in the UI.
      const depth = Math.min(Math.floor(indent.length / 2), 1);
      const item: ChangelogItem = { text, depth };
      if (group) {
        group.items.push(item);
      } else {
        // A bullet before any `###` is prose in list form; keep it in the
        // summary rather than inventing an untitled group.
        entry.summary.push(text);
      }
      openItem = item;
      paragraph = null;
      continue;
    }

    if (line.trim() === "") {
      closeBlocks();
      continue;
    }

    // A plain line: it continues whatever is open, or starts a paragraph.
    if (openItem) {
      openItem.text = `${openItem.text} ${line.trim()}`;
      continue;
    }

    if (paragraph) {
      const target = paragraph.target;
      target[paragraph.index] = `${target[paragraph.index]} ${line.trim()}`;
      continue;
    }

    if (entry && group) {
      // Prose directly under a group heading with no bullet — treated as an
      // item so it is not silently dropped.
      group.items.push({ text: line.trim(), depth: 0 });
      continue;
    }

    const target = entry ? entry.summary : preamble;
    target.push(line.trim());
    paragraph = { target, index: target.length - 1 };
  }

  // A `# Title` heading is a document title, not content.
  return {
    preamble: preamble.filter((text) => text !== "" && !text.startsWith("# ")),
    entries,
  };
}

/** Locates the entry matching a version, so the UI can mark where the reader is. */
export function findEntry(changelog: Changelog, version: string): ChangelogEntry | null {
  return changelog.entries.find((entry) => entry.version === version) ?? null;
}

// --- inline markup ---------------------------------------------------------

export type InlineSegment =
  | { kind: "text"; text: string }
  | { kind: "strong"; text: string }
  | { kind: "code"; text: string }
  | { kind: "link"; text: string; href: string };

/**
 * Splits one line of changelog prose into renderable pieces.
 *
 * Handles the three inline forms a changelog actually uses — `**bold**`,
 * `` `code` `` and `[text](url)` — and leaves everything else as text. An
 * unterminated marker is text, not a parse error: the reader sees the source
 * character they would see in an editor.
 *
 * Links are the one segment carrying a URL. `safeHref` is applied by the
 * renderer, not here, so this function stays pure string handling.
 */
export function parseInline(text: string): InlineSegment[] {
  const segments: InlineSegment[] = [];
  let buffer = "";
  let index = 0;

  const flush = () => {
    if (buffer !== "") {
      segments.push({ kind: "text", text: buffer });
      buffer = "";
    }
  };

  while (index < text.length) {
    const rest = text.slice(index);

    const link = /^\[([^\]]*)\]\(([^)\s]+)\)/.exec(rest);
    if (link) {
      flush();
      segments.push({ kind: "link", text: link[1], href: link[2] });
      index += link[0].length;
      continue;
    }

    const strong = /^\*\*([^*]+)\*\*/.exec(rest);
    if (strong) {
      flush();
      segments.push({ kind: "strong", text: strong[1] });
      index += strong[0].length;
      continue;
    }

    const code = /^`([^`]+)`/.exec(rest);
    if (code) {
      flush();
      segments.push({ kind: "code", text: code[1] });
      index += code[0].length;
      continue;
    }

    buffer += text[index];
    index += 1;
  }

  flush();
  return segments;
}

/**
 * A URL safe to put in an `href`, or null.
 *
 * `javascript:` and `data:` URLs in a link are the classic way a text renderer
 * becomes an execution primitive. The changelog is ours, but the release body in
 * the update manifest is fetched over the network, so the check belongs here
 * rather than in an assumption about who wrote the text.
 */
export function safeHref(href: string): string | null {
  const trimmed = href.trim();
  if (/^(https?:|mailto:)/i.test(trimmed)) return trimmed;
  // A relative link is meaningless in a packaged app with no origin.
  return null;
}
