import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useBranchesStore } from "./branches";
import { useReposStore } from "./repos";
import * as commands from "../bridges/commands";

vi.mock("../bridges/commands", () => ({
  branchList: vi.fn(),
  branchCheckout: vi.fn(),
  branchCheckoutRemote: vi.fn(),
  branchCreate: vi.fn(),
  branchRename: vi.fn(),
  branchDelete: vi.fn(),
  repositoryRefresh: vi.fn(),
  repositoryRemotesLoad: vi.fn(),
  wcEntryOpen: vi.fn(),
}));

const emptyPage = [] as never;

/** The stored repository record, with HEAD on `main`. */
const record = {
  id: "repo-1",
  gitdir: "/tmp/repo-1.git",
  workdir: "/tmp/repo-1",
  isBare: false,
  isEmpty: false,
  headBranch: "main",
  headOid: "abc123",
  state: "clean",
  discovered: false,
} as const;

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  vi.mocked(commands.branchList).mockResolvedValue(emptyPage);
  vi.mocked(commands.repositoryRefresh).mockResolvedValue({ ...record, headBranch: "feature" });
});

/** Select a repository the store actually holds, so `selected` is populated. */
function openRepo(): void {
  const repos = useReposStore();
  repos.upsert({ ...record });
  repos.select("repo-1");
}

describe("branch verbs re-read HEAD", () => {
  it("a checkout refreshes the repository's HEAD", async () => {
    vi.mocked(commands.branchCheckout).mockResolvedValue({
      branch: "feature",
      oid: "abc123",
      detached: false,
    });
    const branches = useBranchesStore();
    openRepo();

    await branches.checkout("repo-1", "feature");

    expect(commands.repositoryRefresh).toHaveBeenCalledWith("repo-1");
  });

  it("creating a branch without checking out leaves HEAD alone", async () => {
    vi.mocked(commands.branchCreate).mockResolvedValue({
      name: "topic",
      fullName: "refs/heads/topic",
      oid: "abc123",
      isHead: false,
      isRemote: false,
      upstream: undefined,
      ahead: 0,
      behind: 0,
      tipSummary: "",
      tipTime: 0,
    });
    const branches = useBranchesStore();

    await branches.create("repo-1", "topic", false);

    expect(commands.repositoryRefresh).not.toHaveBeenCalled();
  });

  it("creating a branch with checkout refreshes HEAD", async () => {
    vi.mocked(commands.branchCreate).mockResolvedValue({
      name: "topic",
      fullName: "refs/heads/topic",
      oid: "abc123",
      isHead: true,
      isRemote: false,
      upstream: undefined,
      ahead: 0,
      behind: 0,
      tipSummary: "",
      tipTime: 0,
    });
    const branches = useBranchesStore();

    await branches.create("repo-1", "topic", true);

    expect(commands.repositoryRefresh).toHaveBeenCalledWith("repo-1");
  });

  it("deleting the current branch refreshes HEAD", async () => {
    vi.mocked(commands.branchDelete).mockResolvedValue({} as never);
    const branches = useBranchesStore();
    openRepo();

    await branches.remove("repo-1", ["main"]);

    // The record still names `main`, so deleting it must invalidate that name.
    expect(commands.repositoryRefresh).toHaveBeenCalledWith("repo-1");
  });

  it("deleting some other branch leaves HEAD alone", async () => {
    vi.mocked(commands.branchDelete).mockResolvedValue({} as never);
    const branches = useBranchesStore();
    openRepo();

    await branches.remove("repo-1", ["feature"]);

    expect(commands.repositoryRefresh).not.toHaveBeenCalled();
  });
});