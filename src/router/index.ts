/**
 * A minimal hash router (no dependency beyond Vue).
 *
 * Why not vue-router: the window is a single document with five screens and a
 * handful of query values; a matcher is ~100 lines and keeps the dependency
 * surface of the webview app small (docs/02 §1 lists the frontend stack).
 *
 * The router is deliberately presentation-free: `App.vue` switches on
 * `route.name` and renders the screen components.
 */

import { ref, type Ref } from "vue";

import { ROUTE_DEFINITIONS, repositoriesPath, type RouteName } from "./routes";

export interface RouteLocation {
  name: RouteName;
  path: string;
  params: Record<string, string>;
  query: Record<string, string>;
}

function readHash(): string {
  const raw = window.location.hash.replace(/^#/, "");
  return raw === "" ? repositoriesPath() : raw;
}

function parseQuery(raw: string): Record<string, string> {
  const query: Record<string, string> = {};
  for (const [key, value] of new URLSearchParams(raw)) query[key] = value;
  return query;
}

export function parseLocation(input: string): RouteLocation {
  const queryIndex = input.indexOf("?");
  const path = queryIndex === -1 ? input : input.slice(0, queryIndex);
  const query = parseQuery(queryIndex === -1 ? "" : input.slice(queryIndex + 1));
  const segments = path.split("/").filter(Boolean).map(decodeURIComponent);

  for (const definition of ROUTE_DEFINITIONS) {
    const pattern = definition.path.split("/").filter(Boolean);
    if (pattern.length !== segments.length) continue;
    const params: Record<string, string> = {};
    let matched = true;
    for (let index = 0; index < pattern.length; index += 1) {
      const part = pattern[index];
      const value = segments[index];
      if (part.startsWith(":")) {
        params[part.slice(1)] = value;
      } else if (part !== value) {
        matched = false;
        break;
      }
    }
    if (matched) return { name: definition.name, path, params, query };
  }

  return { name: "repositories", path: repositoriesPath(), params: {}, query: {} };
}

const route = ref<RouteLocation>(parseLocation(readHash()));

function sync(): void {
  route.value = parseLocation(readHash());
}

window.addEventListener("hashchange", sync);

/** Normalise an empty hash so the first render has a real location. */
if (window.location.hash === "") {
  window.history.replaceState(null, "", `#${repositoriesPath()}`);
}

export function useRoute(): Ref<RouteLocation> {
  return route;
}

/** Navigate; pushes a history entry unless `replace` is set. */
export function navigate(target: string, options: { replace?: boolean } = {}): void {
  const hash = target.startsWith("#") ? target : `#${target}`;
  if (window.location.hash === hash) {
    sync();
    return;
  }
  if (options.replace) {
    window.history.replaceState(null, "", hash);
    sync();
    return;
  }
  window.location.hash = hash;
}

export function goBack(): void {
  window.history.back();
}
