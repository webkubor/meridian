/**
 * `loadState.ts` — the four-state contract (docs/04 §1.2) every async list
 * renders: `loading | empty | error | data`. These tests cover the two pure
 * constructors the stores use to produce those states, and the narrowing a
 * consumer performs on the result.
 */

import { describe, expect, it } from "vitest";

import type { ErrorPayload } from "../bridges/bindings";
import { dataState, errorState, type LoadState } from "./loadState";

const indexLocked: ErrorPayload = {
  code: "index_locked",
  message: "Another Git process is using this repository.",
  detail: "index.lock exists",
  params: { path: "/tmp/repo/.git/index.lock" },
  retryable: true,
  domain: "git",
};

/**
 * The consumer side of the contract: a component that forgets an arm fails to
 * compile (`strict`) rather than rendering nothing. Exercising the narrowed
 * arms here is what proves the constructors below produce values a consumer
 * can actually switch on.
 */
function describeState(state: LoadState<string[]>): string {
  switch (state.status) {
    case "loading":
      return "loading";
    case "empty":
      return "empty";
    case "error":
      return `error:${state.error.code}`;
    case "data":
      return `data:${state.data.length}`;
  }
}

describe("dataState", () => {
  it("turns an empty list into `empty`, never into `data` with no rows", () => {
    // The distinction matters: an empty table and a loading table look
    // different to the user, and only Rust knows whether a load is still
    // running — an empty array must not be promoted to "data".
    const state = dataState<string>([]);
    expect(state).toEqual({ status: "empty" });
    expect(state).not.toHaveProperty("data");
  });

  it("carries a non-empty list through unchanged, in the order given", () => {
    const rows = ["ccc", "aaa", "bbb"];
    const state = dataState(rows);
    expect(state).toEqual({ status: "data", data: rows });
    expect(rows).toEqual(["ccc", "aaa", "bbb"]);
    expect(describeState(state)).toBe("data:3");
  });

  it("decides on length alone — one falsy row is still data", () => {
    expect(dataState([""]).status).toBe("data");
    expect(describeState(dataState([""]))).toBe("data:1");
  });
});

describe("errorState", () => {
  it("wraps an ErrorPayload as the error arm, by reference", () => {
    const state = errorState(indexLocked);
    expect(state.status).toBe("error");
    expect(state).toEqual({ status: "error", error: indexLocked });
    if (state.status === "error") {
      // Not a copy: `message`/`params` reach the error panel exactly as the
      // backend sent them.
      expect(state.error).toBe(indexLocked);
      expect(state.error.retryable).toBe(true);
    }
    expect(describeState(state)).toBe("error:index_locked");
  });

  it("hands anything thrown through as-is — it is a cast, not a converter", () => {
    // The signature takes `unknown` and casts to ErrorPayload; nothing here
    // normalises a raw `Error` into a code/message envelope. A caller that
    // catches must therefore pass the wire payload, and this test pins that
    // behaviour so a silent conversion is never assumed.
    const thrown = new Error("boom");
    const state = errorState(thrown);
    expect(state).toEqual({ status: "error", error: thrown });
  });
});
