/**
 * The renderer's side of the failure net.
 *
 * The guarantee under test is the one the net depends on: a failure is silent
 * only while something on screen is showing it, and the toast fires when
 * nothing is. Claiming at the component that renders `state.error` is what makes
 * "the pane is not mounted" mean "the user is told".
 */

import { beforeEach, describe, expect, it } from "vitest";
import { defineComponent, h, nextTick, type PropType } from "vue";
import { mount } from "@vue/test-utils";

import type { ErrorPayload } from "../bridges/bindings";
import { useFailureClaim } from "./claim";
import { captureFailure, configureFailureNet, resetFailureNet } from "./failures";

function gitError(): ErrorPayload {
  return { code: "checkout_conflicts", message: "local changes would be overwritten", retryable: false, domain: "git" };
}

/** The app's own net, with a controllable clock so the grace is arithmetic. */
const shown: ErrorPayload[] = [];
let timers: Array<() => void> = [];

beforeEach(() => {
  resetFailureNet();
  shown.length = 0;
  timers = [];
  configureFailureNet({
    graceMs: 250,
    schedule: (callback) => {
      timers.push(callback);
      return timers.length - 1;
    },
    cancel: (handle) => {
      timers[handle as number] = () => {};
    },
    showToast: (payload) => shown.push(payload),
  });
});

function fire(): void {
  for (const callback of timers.splice(0)) callback();
}

/** Stands in for `<ListShell>`: mounted, with an error prop, claiming it. */
const Pane = defineComponent({
  props: { error: { type: Object as PropType<ErrorPayload | null>, default: null } },
  setup(props) {
    useFailureClaim(() => props.error);
    return () => h("p", props.error ? "error arm" : "list");
  },
});

describe("useFailureClaim", () => {
  it("claims the failure while the component is mounted, so no toast is raised", async () => {
    const error = gitError();
    const wrapper = mount(Pane, { props: { error: null } });

    // A failure nobody claimed yet, then the pane receives it: the claim has to
    // land before the grace period expires.
    captureFailure(error, "branch_list");
    await wrapper.setProps({ error });
    await nextTick();
    fire();

    expect(shown).toHaveLength(0);
    expect(wrapper.text()).toBe("error arm");
  });

  it("leaves the failure pending when nothing is mounted to claim it", () => {
    // The history pane is not mounted on the working-copy tab, so a failed
    // history load must still reach the user.
    captureFailure(gitError(), "history_commits_load");
    fire();
    expect(shown).toHaveLength(1);
  });

  it("claims on the very first render, before any watcher flush", () => {
    const error = gitError();
    captureFailure(error, "branch_list");
    // Mounted with the error already in its props, which is the common case:
    // a pane that renders an existing error state.
    mount(Pane, { props: { error } });
    fire();
    expect(shown).toHaveLength(0);
  });
});
