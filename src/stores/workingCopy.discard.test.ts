import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useWorkingCopyStore } from "./workingCopy";
import * as commands from "../bridges/commands";

vi.mock("../bridges/commands", () => ({
  wcPathsDiscard: vi.fn(),
  wcStatusLoad: vi.fn(),
}));

describe("discard", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("includes the untracked delete intent only after the caller's confirmation", async () => {
    vi.mocked(commands.wcPathsDiscard).mockResolvedValue({
      repo: "repo-1",
      entries: [],
      summary: { staged: 0, unstaged: 0, untracked: 0, conflicted: 0 },
    });
    const store = useWorkingCopyStore();

    const ok = await store.discard("repo-1", ["scratch.txt"], true);

    expect(ok).toBe(true);
    expect(commands.wcPathsDiscard).toHaveBeenCalledWith("repo-1", ["scratch.txt"], {
      includeUntracked: true,
      confirmToken: "confirmed",
    });
  });

  it("never sets includeUntracked for a tracked path", async () => {
    vi.mocked(commands.wcPathsDiscard).mockResolvedValue({
      repo: "repo-1",
      entries: [],
      summary: { staged: 0, unstaged: 0, untracked: 0, conflicted: 0 },
    });
    const store = useWorkingCopyStore();

    await store.discard("repo-1", ["tracked.txt"]);

    expect(commands.wcPathsDiscard).toHaveBeenCalledWith("repo-1", ["tracked.txt"], {});
  });

  it("returns failure and retains the error if the backend refuses a discard", async () => {
    const error = {
      code: "invalid_request",
      domain: "git",
      message: "Untracked file requires confirmation.",
      retryable: false,
    } as const;
    vi.mocked(commands.wcPathsDiscard).mockRejectedValue(error);
    const store = useWorkingCopyStore();

    const ok = await store.discard("repo-1", ["scratch.txt"]);

    expect(ok).toBe(false);
    expect(store.lastError).toEqual(error);
  });
});