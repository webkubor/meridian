/**
 * The commit-list module's public surface.
 *
 * `VirtualCommitList` is the pane's list; `CommitRow` is its row component (also
 * usable by the file-history/PR timelines that share the commit cell shape);
 * `rows.ts` is the display-row model the pane can reuse for counting and
 * "show in history" scrolling.
 */

export { default as VirtualCommitList } from "./VirtualCommitList.vue";
export { default as CommitRow } from "./CommitRow.vue";

export {
  buildDisplayRows,
  computeOffsets,
  findRowAt,
  formatExact,
  formatGroupLabel,
  formatWhen,
  groupKeyFor,
  isCommitRow,
  GROUP_ROW_HEIGHT,
  ROW_HEIGHT,
  type BuildRowsOptions,
  type CommitDisplayRow,
  type CommitGraphRowLane,
  type CommitRowKind,
  type DateGrouping,
  type RowGraph,
} from "./rows";

export { commitListStrings } from "./strings";
