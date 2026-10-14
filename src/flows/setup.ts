/**
 * jsdom 宿主 API 垫片与 Tauri 外部边界 mock
 * 路由：src/flows 流程测试
 * API：为真实 Vue 应用提供浏览器宿主与可编程 Tauri 命令后端
 */
import { beforeEach, vi } from "vitest";

import { backend } from "./backend";

const opener = vi.hoisted(() => ({ calls: [] as string[], fail: false }));
const clipboard = vi.hoisted(() => ({ writes: [] as string[] }));



beforeEach(() => {
  opener.calls.splice(0);
  opener.fail = false;
  clipboard.writes.splice(0);
});

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: async (url: string) => {
    opener.calls.push(url);
    if (opener.fail) throw new Error("no handler registered for this URL");
  },
}));



export { backend };

/**
 * A ResizeObserver that reports an initial observation.
 *
 * A real one delivers its first callback asynchronously after `observe()`.
 * An inert stub leaves every measured height at zero, which silently stops the
 * virtualised lists from ever reaching their paging threshold — the test would
 * then pass for the wrong reason.
 */
class TestResizeObserver {
  private targets = new Set<Element>();

  constructor(private readonly callback: ResizeObserverCallback) {}

  observe(target: Element): void {
    this.targets.add(target);
    setTimeout(() => this.deliver(), 0);
  }

  unobserve(target: Element): void {
    this.targets.delete(target);
  }

  disconnect(): void {
    this.targets.clear();
  }

  private deliver(): void {
    if (this.targets.size === 0) return;
    const entries = [...this.targets].map((target) => ({
      target,
      contentRect: target.getBoundingClientRect(),
      borderBoxSize: [],
      contentBoxSize: [],
      devicePixelContentBoxSize: [],
    })) as unknown as ResizeObserverEntry[];
    this.callback(entries, this as unknown as ResizeObserver);
  }
}

const canvasContext = new Proxy(
  {
    measureText: (text: string) => ({ width: text.length * 7 }),
  } as Record<string, unknown>,
  {
    get(target, property) {
      if (property in target) return target[property as string];
      if (property === "canvas") return document.createElement("canvas");
      return () => undefined;
    },
    set(target, property, value) {
      target[property as string] = value;
      return true;
    },
  },
);

if (typeof window !== "undefined") {
  if (typeof window.matchMedia !== "function") {
    window.matchMedia = ((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => undefined,
      removeListener: () => undefined,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
      dispatchEvent: () => false,
    })) as typeof window.matchMedia;
  }
  if (typeof window.ResizeObserver === "undefined") {
    Object.defineProperty(window, "ResizeObserver", { configurable: true, value: TestResizeObserver });
  }
  if (typeof window.scrollTo !== "function") window.scrollTo = () => undefined;
  if (typeof HTMLElement.prototype.scrollTo !== "function") HTMLElement.prototype.scrollTo = () => undefined;
  if (typeof window.requestAnimationFrame !== "function") {
    window.requestAnimationFrame = (callback) => window.setTimeout(() => callback(Date.now()), 16);
    window.cancelAnimationFrame = (handle) => window.clearTimeout(handle);
  }
  // jsdom has no canvas backend; the graph renderer only needs a context whose
  // drawing calls are no-ops and whose measureText returns a width.
  const nativeGetContext = HTMLCanvasElement.prototype.getContext;
  HTMLCanvasElement.prototype.getContext = function getContext(this: HTMLCanvasElement, kind: string): unknown {
    if (kind === "2d") return canvasContext as unknown as CanvasRenderingContext2D;
    return nativeGetContext.call(this, kind as "2d");
  } as unknown as HTMLCanvasElement["getContext"];

  clipboard.writes.splice(0);
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: async (text: string) => { clipboard.writes.push(text); } },
  });
}

vi.mock("@tauri-apps/api/core", () => ({
  Channel: class<T> {
    onmessage: ((message: T) => void) | undefined;
  },
  invoke: <T>(name: string, payload?: Record<string, unknown>) => backend.invoke<T>(name, payload),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: async () => () => undefined,
}));

/**
 * The two boundaries a test cannot click: the system opener and the clipboard.
 * `vi.hoisted` so the mock factories below can reach them (mock factories are
 * hoisted above imports).
 */
/** The two boundaries a test cannot click: the system opener and the clipboard. */
export const externalBoundaries = { opener, clipboard };
