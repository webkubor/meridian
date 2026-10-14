/**
 * 真实 Meridian 应用的流程测试挂载器
 * 路由：src/flows 测试使用的集成 harness
 * API：真实 App.vue、Pinia、zh-CN i18n、hash router 与诊断失败网
 */
import { beforeEach, vi } from "vitest";
import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createPinia } from "pinia";
import { nextTick } from "vue";

import "./setup";
import {
  backend,
  FakeBackend,
  fixtureBranchName,
  fixtureFilePath,
  fixtureRemoteBranch,
  fixtureRepoId,
  fixtureRepoPath,
  makeError,
} from "./backend";
import { i18n, setLocale } from "../i18n";
import { navigate } from "../router";
import { useShell } from "../layout/useShell";
import { clearRecorder, recentCalls, recentFailures, setRecorderDebug } from "../diagnostics/recorder";
import { installGlobalErrorHandlers } from "../diagnostics/global";
import { resetFailureNet } from "../diagnostics/failures";
import type { ErrorPayload } from "../bridges/bindings";
import App from "../App.vue";

export { backend, FakeBackend, fixtureBranchName, fixtureFilePath, fixtureRemoteBranch, fixtureRepoId, fixtureRepoPath, makeError };
export type { ErrorPayload };
export const failure = (command: string, payload?: Partial<ErrorPayload>): ErrorPayload => FakeBackendFailure(command, payload);

function FakeBackendFailure(command: string, payload?: Partial<ErrorPayload>): ErrorPayload {
  return {
    code: "invalid_request",
    message: `${command} rejected by the test backend`,
    detail: `fixture failure for ${command}`,
    params: {},
    retryable: false,
    domain: "git",
    ...payload,
  };
}

export let app: VueWrapper | null = null;

/**
 * The Vue/window/unhandled-rejection handlers `main.ts` installs at boot.
 *
 * The harness mounts `App.vue` directly, so without this the window-level paths
 * would have no listener and a global failure would vanish — the exact silence
 * these tests exist to rule out.
 */
let uninstallGlobalHandlers: (() => void) | null = null;

/**
 * Return the module-level shell singletons to their start-of-test values.
 *
 * `useShell` keeps its toast queue and dialogue queue in module scope, so
 * unmounting the app does not empty them: without this a dialogue opened by one
 * test is still on screen for the next one, and the next test would be asserting
 * against the previous test's UI.
 */
export function resetShell(): void {
  const shell = useShell();
  for (let pending = shell.activeDialogue.value; pending; pending = shell.activeDialogue.value) {
    shell.cancelDialogue(pending.id);
  }
  for (const toast of [...shell.toasts.value]) shell.dismissToast(toast.id);
  shell.setThemeMode("auto");
}

/** Recreate module-owned UI state and mount the real application shell. */
export async function mountApp(path = "/repos"): Promise<VueWrapper> {
  localStorage.clear();
  setLocale("zh-CN");
  document.documentElement.lang = "zh-CN";
  document.documentElement.className = "";
  document.documentElement.dataset.theme = "light";
  document.documentElement.style.colorScheme = "light";
  resetShell();
  backend.reset();
  clearRecorder();
  resetFailureNet();
  setRecorderDebug(false);
  await vi.advanceTimersByTimeAsync(0);
  navigate(path, { replace: true });
  await nextTick();
  app = mount(App, {
    attachTo: document.body,
    global: { plugins: [createPinia(), i18n] },
  });
  uninstallGlobalHandlers = installGlobalErrorHandlers(app.vm.$.appContext.app);
  await flushFlow();
  return app;
}

/** Resolve Vue updates, promise continuations, animation frames and failure timers. */
export async function flushFlow(advanceMs = 0): Promise<void> {
  await flushPromises();
  await nextTick();
  if (advanceMs > 0) await vi.advanceTimersByTimeAsync(advanceMs);
  await flushPromises();
  await nextTick();
  // The router reacts to `hashchange`, which jsdom queues as a task that fake
  // timers never run: without this a navigation updates the URL but leaves the
  // previous screen mounted, which is exactly the class of bug these tests
  // exist to catch.
  window.dispatchEvent(new HashChangeEvent("hashchange"));
  await flushPromises();
  await nextTick();
  // Resize observation arrives on a timer; the virtual lists need it before
  // they can measure themselves and ask for another page.
  await vi.advanceTimersByTimeAsync(1);
  await flushPromises();
  await nextTick();
}

/** Open the repository fixture through the actual Open control. */
export async function openRepositoryFixture(wrapper = app): Promise<void> {
  if (!wrapper) throw new Error("Call mountApp() before opening a repository fixture");
  const input = wrapper.get<HTMLInputElement>("input[aria-label='要打开的文件夹']");
  await input.setValue("/tmp/meridian-flow");
  await wrapper.get("form").trigger("submit");
  await flushFlow();
}

/**
 * Switch the repository workspace tab.
 *
 * Tab names repeat in the toolbar and the sidebar ("分支" is a toolbar verb, a
 * tab and a sidebar section), so the tab strip is addressed by ARIA role rather
 * than by text.
 */
export async function clickWorkspaceTab(label: string, wrapper = app): Promise<void> {
  if (!wrapper) throw new Error("Call mountApp() before switching tabs");
  const tab = wrapper.findAll('[role="tab"]').find((candidate) => candidate.text().trim() === label);
  if (!tab) throw new Error(`No workspace tab labelled ${JSON.stringify(label)}`);
  await tab.trigger("click");
  await flushFlow();
}

/**
 * Click a toolbar verb's primary button.
 *
 * The toolbar is scoped because the same words appear in the sidebar (储藏 is a
 * sidebar section as well as a toolbar verb).
 */
export async function clickToolbarVerb(label: string, wrapper = app): Promise<void> {
  if (!wrapper) throw new Error("Call mountApp() before clicking controls");
  const button = wrapper.findAll(".toolbar .split-primary").find((candidate) => candidate.text().trim() === label);
  if (!button) throw new Error(`The toolbar has no ${JSON.stringify(label)} verb`);
  await button.trigger("click");
  await flushFlow();
}

/** Click one unique visible button by its exact translated label. */
export async function clickButton(label: string, wrapper = app): Promise<void> {
  if (!wrapper) throw new Error("Call mountApp() before clicking controls");
  const buttons = wrapper.findAll("button").filter((button) => button.text().trim() === label);
  if (buttons.length !== 1) throw new Error(`Expected one button labelled ${JSON.stringify(label)}; found ${buttons.length}`);
  await buttons[0]!.trigger("click");
  await flushFlow();
}

/** Click the first element matching a CSS selector inside the shell or its teleports. */
export async function clickSelector(selector: string, wrapper = app): Promise<void> {
  if (!wrapper) throw new Error("Call mountApp() before clicking controls");
  const target = wrapper.element.closest("body")?.querySelector(selector) ?? wrapper.get(selector);
  if (!(target instanceof Element)) throw new Error(`No element matched ${JSON.stringify(selector)}`);
  target.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  await flushFlow();
}

/**
 * Toast text, read from the document.
 *
 * `<ToastHost>` teleports into `body`, so `wrapper.text()` cannot see a toast —
 * which would hide exactly the assertion this suite exists to make.
 */
export function toasts(kind?: "danger" | "success" | "warning" | "info"): string[] {
  const selector = kind ? `.toast.kind-${kind}` : ".toast";
  return [...document.querySelectorAll(selector)].map((node) => node.textContent?.trim() ?? "");
}

/** True when some danger toast contains `text`. */
export function hasDangerToast(text: string): boolean {
  return toasts("danger").some((toast) => toast.includes(text));
}

/** True when some success toast contains `text`. */
export function hasSuccessToast(text: string): boolean {
  return toasts("success").some((toast) => toast.includes(text));
}

/** The active modal dialogue, which also renders through a body-level teleport. */
export function dialogue(): HTMLElement {
  const panel = document.querySelector('[role="dialog"]');
  if (!(panel instanceof HTMLElement)) throw new Error("No dialogue is open");
  return panel;
}

/** Text of the first visible inline error (list error state or launch alert). */
export function alertText(): string {
  const alert = document.querySelector('[role="alert"]');
  if (!(alert instanceof HTMLElement)) throw new Error("No inline error is rendered");
  return alert.textContent?.trim() ?? "";
}

/** Type a value into the open dialogue's single-line field. */
export async function typeInDialogue(value: string): Promise<void> {
  const input = dialogue().querySelector("input.prompt-input");
  if (!(input instanceof HTMLInputElement)) throw new Error("The open dialogue has no text field");
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  await flushFlow();
}

/** Tick or untick the open dialogue's option checkbox. */
export async function checkDialogueOption(checked: boolean): Promise<void> {
  await setChecked('.dialogue input[type="checkbox"]', checked);
}

/** Confirm the open dialogue with its primary button. */
export async function confirmDialogue(): Promise<void> {
  await clickInDialogue("button.btn.primary");
}

/** Pick a row inside the open choose-branch dialogue by its visible text. */
export async function pickBranchInDialogue(text: string): Promise<void> {
  const row = dialogue().querySelectorAll('[role="option"]');
  const match = [...row].find((option) => option.textContent?.includes(text));
  if (!(match instanceof HTMLElement)) throw new Error(`No branch ${JSON.stringify(text)} in the dialogue`);
  match.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  await flushFlow();
}

/**
 * Scroll a virtualised list to its end, as a user would.
 *
 * jsdom reports `scrollHeight === clientHeight === 0`, so assigning `scrollTop`
 * is silently discarded and the list never moves; the event has to be dispatched
 * directly for the component to see the window reach the tail.
 */
export async function scrollListToEnd(selector: string): Promise<void> {
  const list = document.querySelector(selector);
  if (!(list instanceof HTMLElement)) throw new Error(`No scrollable list matched ${JSON.stringify(selector)}`);
  list.scrollTop = Number.MAX_SAFE_INTEGER;
  list.dispatchEvent(new Event("scroll", { bubbles: true }));
  // The list coalesces its scroll handler into one animation frame, and the
  // paging request it raises resolves in the microtasks after that.
  await vi.advanceTimersByTimeAsync(64);
  await flushFlow();
  await vi.advanceTimersByTimeAsync(64);
  await flushFlow();
}

/** Click a control inside the open dialogue. */
export async function clickInDialogue(selector: string): Promise<void> {
  const target = dialogue().querySelector(selector);
  if (!(target instanceof Element)) throw new Error(`No dialogue control matched ${JSON.stringify(selector)}`);
  target.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  await flushFlow();
}

/** Set the value of an input, firing the events Vue listens for. */
export async function setInput(selector: string, value: string, wrapper = app): Promise<void> {
  if (!wrapper) throw new Error("Call mountApp() before filling controls");
  const input = wrapper.element.closest("body")?.querySelector(selector) ?? wrapper.get(selector);
  if (!(input instanceof HTMLInputElement)) throw new Error(`No input matched ${JSON.stringify(selector)}`);
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  await flushFlow();
}

/** Toggle a checkbox by selector, as a user would. */
export async function setChecked(selector: string, checked: boolean): Promise<void> {
  const box = document.querySelector(selector);
  if (!(box instanceof HTMLInputElement)) throw new Error(`No checkbox matched ${JSON.stringify(selector)}`);
  box.checked = checked;
  box.dispatchEvent(new Event("change", { bubbles: true }));
  await flushFlow();
}

/** Mark the named invoke failure handled and tick the app's grace timer. */

export async function settleFailure(command: string, payload?: Partial<ErrorPayload>): Promise<void> {
  backend.failNext(command, failure(command, payload));
  await flushFlow(300);
}

/** Return a deep copy of all backend IPC calls. */
export function recordedCalls() {
  return backend.calls.map((call) => ({ name: call.name, payload: structuredClone(call.payload) }));
}

/** Return the recorder state exposed to the diagnostics screen. */
export function recordedFailures() {
  return [...recentFailures()];
}

/** Return frontend call history exposed to diagnostics. */
export function recordedBridgeCalls() {
  return [...recentCalls()];
}

/** Unmount the shell and reset timer state for the next test. */
export async function unmountApp(): Promise<void> {
  uninstallGlobalHandlers?.();
  uninstallGlobalHandlers = null;
  app?.unmount();
  app = null;
  resetShell();
  document.body.innerHTML = "";
  await flushPromises();
}

beforeEach(async () => {
  if (app) await unmountApp();
});
