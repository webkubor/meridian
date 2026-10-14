/**
 * Pure mappings from the Rust command payloads (`bridges/bindings.ts`) onto the
 * UI view models (`components/contracts.ts`).
 *
 * The panes are presentational and render exactly the contracts shapes; the
 * stores own the translation, so no component ever has to know a wire field
 * name. Nothing here touches Vue or Tauri — the functions are total and pure.
 */

import type {
  Changeset as ChangesetWire,
  CommitDetail as CommitDetailWire,
  CommitSignatureState,
  CommitInfo,
  DiffDocument as DiffDocumentWire,
  DiffOptionsWire,
  DiffPerspective as DiffPerspectiveWire,
  GraphLayout as GraphLayoutWire,
  RefDecorationKind,
} from "../bridges/bindings";
import type {
  Changeset,
  CommitDetail,
  CommitSignature,
  DiffDocument,
  DiffOptions,
  DiffPerspective,
  DiffRow,
  GraphLaneSegment,
  GraphLayout,
  HunkRef,
  RefChip,
  SignatureState,
} from "../components/contracts";

/* ---- commits ------------------------------------------------------------- */

function refKind(kind: RefDecorationKind): RefChip["kind"] {
  switch (kind) {
    case "head":
      return "head";
    case "tag":
      return "tag";
    case "remote":
      return "remoteBranch";
    case "branch":
      return "branch";
    default:
      return "other";
  }
}

/**
 * The engine reports *presence* and *format*, never trust (verification is a
 * separate command that has not landed). A present-but-unverified signature is
 * shown as exactly that — never as "valid".
 */
function signatureState(state: CommitSignatureState): SignatureState {
  return state === "unsigned" ? "unsigned" : "present";
}

export function toCommitDetail(wire: CommitDetailWire): CommitDetail {
  const commit: CommitInfo = {
    oid: wire.oid,
    shortOid: wire.shortOid,
    parents: wire.parents,
    summary: wire.summary,
    message: wire.message,
    author: wire.author,
    committer: wire.committer,
    decorations: wire.decorations,
  };
  const refs: RefChip[] = wire.refs.map((ref) => ({ name: ref.name, kind: refKind(ref.kind) }));
  const signature: CommitSignature = { state: signatureState(wire.signature) };
  return { commit, refs, signature };
}

export function toChangeset(wire: ChangesetWire): Changeset {
  return {
    entries: wire.entries.map((entry) => ({
      path: entry.path,
      oldPath: entry.oldPath ?? null,
      status: entry.status,
      additions: entry.insertions,
      deletions: entry.deletions,
      binary: entry.binary,
    })),
    stats: {
      files: wire.stats.filesChanged,
      additions: wire.stats.insertions,
      deletions: wire.stats.deletions,
    },
  };
}

/* ---- graph --------------------------------------------------------------- */

export function toGraphLayout(wire: GraphLayoutWire): GraphLayout {
  const segments: GraphLaneSegment[] = wire.segments.map((segment) => ({
    lane: segment.lane,
    toLane: segment.toLane,
    fromRow: segment.fromRow,
    toRow: segment.toRow,
    kind: segment.kind,
  }));
  return { laneCount: wire.laneCount, segments, lanes: wire.lanes };
}

/* ---- diff ---------------------------------------------------------------- */

export function toDiffPerspective(perspective: DiffPerspectiveWire): DiffPerspective {
  switch (perspective) {
    case "indexToBase":
      return "index-vs-head";
    case "workingTreeToIndex":
      return "workdir-vs-index";
    case "workingTreeToBase":
      return "workdir-vs-head";
    default:
      return "merge-base";
  }
}

export function toDiffOptionsWire(options: DiffOptions): DiffOptionsWire {
  return {
    contextLines: options.contextLines,
    ignoreWhitespace: options.ignoreWhitespace,
    showFull: options.fullFile,
  };
}

export function toDiffDocument(wire: DiffDocumentWire, fallbackPath: string | null): DiffDocument {
  const rows: DiffRow[] = [];
  for (const hunk of wire.hunks) {
    const ref: HunkRef = {
      oldStart: hunk.oldStart,
      oldLines: hunk.oldLines,
      newStart: hunk.newStart,
      newLines: hunk.newLines,
    };
    rows.push({ kind: "hunkHeader", header: hunk.header, hunk: ref, oldLine: null, newLine: null });
    for (const line of hunk.lines) {
      if (line.kind === "noNewline") {
        rows.push({ kind: "noNewline", side: "new" });
        continue;
      }
      rows.push({
        kind: line.kind,
        text: line.text,
        oldLine: line.oldLineno ?? null,
        newLine: line.newLineno ?? null,
      });
    }
  }

  return {
    path: wire.path ?? fallbackPath ?? "",
    oldPath: wire.change?.oldPath ?? null,
    perspective: toDiffPerspective(wire.perspective),
    rows,
    stats: { additions: wire.additions, deletions: wire.deletions },
    binary: wire.isBinary || undefined,
  };
}
