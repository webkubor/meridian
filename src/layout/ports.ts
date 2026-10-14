/**
 * Ports — the store/composable surfaces this UI layer expects from modules it
 * does not own.
 *
 * The UI is presentational: every screen receives its data through props typed
 * by `components/contracts.ts`, and `App.vue` (the composition root) is the
 * only place that talks to stores/composables. The interfaces below name what
 * that root needs, so the owning module can satisfy it without reading the
 * screens:
 *
 *   graph       → `stores/history.ts` + `composables/useHistory.ts` additions
 *                 (history_graph_compute) — today the graph module
 *                 (`components/graph`) already owns the drawing; the history
 *                 pane only needs the layout it consumes.
 *   diff        → `stores/diff.ts` + `composables/useDiff.ts`
 *   compare     → `stores/compare.ts` + `composables/useCompare.ts`
 *   blame       → `stores/blame.ts` + `composables/useBlame.ts`
 *   settings    → `stores/settings.ts` + `composables/useSettings.ts`
 *   services    → `stores/services.ts` + `composables/useServices.ts`
 *   activities  → `stores/activities.ts` + `composables/useActivities.ts`
 *   merge       → `stores/workingCopy.ts` additions (wc_merge_status_load)
 *
 * Until those land, `App.vue` renders the corresponding screens with an
 * explicit `loading`/`empty` state derived from the port being absent, so no
 * screen shows invented data.
 */

import type { Ref, ComputedRef } from "vue";

import type {
  ActivityItem,
  BlameLines,
  BlamePopover,
  Comparison,
  DiffDocument,
  DiffOptions,
  GraphLayout,
  MergeStatus,
  ServiceAccount,
  SettingsDocument,
  SettingValue,
} from "../components/contracts";
import type { ErrorPayload, RepoId } from "../bridges/bindings";
import type { LoadState } from "../stores/loadState";

/* ---- diff ---------------------------------------------------------------- */

export interface DiffPort {
  document: Ref<LoadState<DiffDocument>>;
  options: Ref<DiffOptions>;
  load(repo: RepoId, path: string): Promise<void>;
  setOptions(options: Partial<DiffOptions>): void;
  stageHunk(repo: RepoId, path: string, hunkIndex: number): Promise<void>;
  unstageHunk(repo: RepoId, path: string, hunkIndex: number): Promise<void>;
  discardHunk(repo: RepoId, path: string, hunkIndex: number): Promise<void>;
}

/* ---- graph --------------------------------------------------------------- */

export interface GraphPort {
  /** `history_graph_compute` over the loaded commit oids. */
  layout: Ref<GraphLayout | null>;
  load(repo: RepoId, oids: string[]): Promise<void>;
}

/* ---- compare ------------------------------------------------------------- */

export interface ComparePort {
  comparison: Ref<LoadState<Comparison>>;
  load(repo: RepoId, left: string, right: string): Promise<void>;
}

/* ---- blame --------------------------------------------------------------- */

export interface BlamePort {
  lines: Ref<LoadState<BlameLines>>;
  /** The annotation popover currently shown, if any. */
  popover: Ref<BlamePopover | null>;
  load(repo: RepoId, path: string, rev?: string): Promise<void>;
  openPopover(line: number): void;
  closePopover(): void;
}

/* ---- settings ------------------------------------------------------------ */

export interface SettingsPort {
  document: Ref<LoadState<SettingsDocument>>;
  load(): Promise<void>;
  set(key: string, value: SettingValue): Promise<void>;
}

/* ---- services / accounts ------------------------------------------------- */

export interface ServicesPort {
  accounts: Ref<LoadState<ServiceAccount[]>>;
  lastError: Ref<ErrorPayload | null>;
  load(): Promise<void>;
}

/* ---- activities ---------------------------------------------------------- */

export interface ActivitiesPort {
  items: Ref<ActivityItem[]>;
  running: ComputedRef<number>;
  stalled: ComputedRef<boolean>;
  cancel(id: string): Promise<void>;
}

/* ---- merge status (working-copy banner) ---------------------------------- */

export interface MergePort {
  status: Ref<MergeStatus | null>;
  load(repo: RepoId): Promise<void>;
}
