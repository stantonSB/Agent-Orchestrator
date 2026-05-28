import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SessionPanel } from "./SessionPanel";
import type { SessionInfo } from "../../types/session";

describe("SessionPanel", () => {
  it("shows empty state when no sessions exist", () => {
    render(
      <SessionPanel
        sessions={[]}
        activeSessionId={null}
        onSessionClick={vi.fn()}
        onNewSession={vi.fn()}
      />
    );
    expect(screen.getByText("No active sessions")).toBeTruthy();
  });

  it("renders session cards for each session", () => {
    const sessions: SessionInfo[] = [
      { id: "1", name: "Session A", status: "working", createdAt: Date.now(), cwd: "/projects/app", sessionType: "claude", isGitRepo: true },
      { id: "2", name: "Session B", status: "idle", createdAt: Date.now(), cwd: "/projects/app", sessionType: "claude", isGitRepo: true },
    ];
    render(
      <SessionPanel
        sessions={sessions}
        activeSessionId="1"
        onSessionClick={vi.fn()}
        onNewSession={vi.fn()}
      />
    );
    expect(screen.getByText("Session A")).toBeTruthy();
    expect(screen.getByText("Session B")).toBeTruthy();
  });

  it("renders the New Session button", () => {
    render(
      <SessionPanel
        sessions={[]}
        activeSessionId={null}
        onSessionClick={vi.fn()}
        onNewSession={vi.fn()}
      />
    );
    expect(screen.getByText("New Session")).toBeTruthy();
  });

  it("groups sessions by project with headers", () => {
    const sessions: SessionInfo[] = [
      { id: "1", name: "Fix bug", status: "working", createdAt: 1000, cwd: "/projects/app-one", sessionType: "claude", isGitRepo: true },
      { id: "2", name: "Add feature", status: "idle", createdAt: 2000, cwd: "/projects/app-two", sessionType: "claude", isGitRepo: true },
    ];
    render(
      <SessionPanel
        sessions={sessions}
        activeSessionId="1"
        onSessionClick={vi.fn()}
        onNewSession={vi.fn()}
      />
    );
    expect(screen.getByText("app-one")).toBeTruthy();
    expect(screen.getByText("app-two")).toBeTruthy();
  });

  it("collapses a project group when header is clicked", () => {
    const sessions: SessionInfo[] = [
      { id: "1", name: "Fix bug", status: "working", createdAt: 1000, cwd: "/projects/app-one", sessionType: "claude", isGitRepo: true },
    ];
    render(
      <SessionPanel
        sessions={sessions}
        activeSessionId={null}
        onSessionClick={vi.fn()}
        onNewSession={vi.fn()}
      />
    );
    expect(screen.getByText("Fix bug")).toBeTruthy();

    fireEvent.click(screen.getByText("app-one"));
    expect(screen.queryByText("Fix bug")).toBeNull();

    fireEvent.click(screen.getByText("app-one"));
    expect(screen.getByText("Fix bug")).toBeTruthy();
  });

  it("orders project groups by newest session first", () => {
    const sessions: SessionInfo[] = [
      { id: "1", name: "Old session", status: "idle", createdAt: 1000, cwd: "/projects/old-project", sessionType: "claude", isGitRepo: true },
      { id: "2", name: "New session", status: "working", createdAt: 3000, cwd: "/projects/new-project", sessionType: "claude", isGitRepo: true },
    ];
    render(
      <SessionPanel
        sessions={sessions}
        activeSessionId={null}
        onSessionClick={vi.fn()}
        onNewSession={vi.fn()}
      />
    );
    const headers = screen.getAllByRole("button").filter(
      (el) => el.getAttribute("aria-expanded") !== null
    );
    expect(headers[0].textContent).toContain("new-project");
    expect(headers[1].textContent).toContain("old-project");
  });

  it("keeps sessions with same folder name but different paths separate", () => {
    const sessions: SessionInfo[] = [
      { id: "1", name: "Work app", status: "working", createdAt: 1000, cwd: "/work/myapp", sessionType: "claude", isGitRepo: true },
      { id: "2", name: "Personal app", status: "idle", createdAt: 2000, cwd: "/personal/myapp", sessionType: "claude", isGitRepo: true },
    ];
    render(
      <SessionPanel
        sessions={sessions}
        activeSessionId={null}
        onSessionClick={vi.fn()}
        onNewSession={vi.fn()}
      />
    );
    // Both should have "myapp" header but be separate groups
    const headers = screen.getAllByText("myapp");
    expect(headers.length).toBe(2);
  });

  it("does not create separate project group for child terminal sessions", () => {
    const sessions: SessionInfo[] = [
      {
        id: "parent-1",
        name: "Claude Parent",
        status: "working",
        createdAt: 1000,
        cwd: "/projects/app",
        sessionType: "claude",
        isGitRepo: true,
      },
      {
        id: "child-1",
        name: "Terminal Child",
        status: "terminal",
        createdAt: 2000,
        cwd: "/projects/app/.claude/worktrees/breezy-frog",
        sessionType: "terminal",
        isGitRepo: false,
        parentSessionId: "parent-1",
      },
    ];
    render(
      <SessionPanel
        sessions={sessions}
        activeSessionId="parent-1"
        onSessionClick={vi.fn()}
        onNewSession={vi.fn()}
      />
    );
    // Should only show the "app" group, not "breezy-frog"
    expect(screen.getByText("app")).toBeTruthy();
    expect(screen.queryByText("breezy-frog")).toBeNull();
    // Child should still be visible (rendered under parent)
    expect(screen.getByText("Terminal Child")).toBeTruthy();
  });

  it("renders subagent entries beneath parent session", async () => {
    const { useSessionStore } = await import("../../stores/sessionStore");
    useSessionStore.setState({
      subagents: new Map([
        ["1", [
          { id: "cc-child-1", index: 1, status: "working", name: "Exploring", created_at: 1000 },
          { id: "cc-child-2", index: 2, status: "idle", name: null, created_at: 2000 },
        ]],
      ]),
    });

    const sessions: SessionInfo[] = [
      { id: "1", name: "Feature work", status: "working", createdAt: 1000, cwd: "/projects/app", sessionType: "claude", isGitRepo: true },
    ];
    render(
      <SessionPanel
        sessions={sessions}
        activeSessionId="1"
        onSessionClick={vi.fn()}
        onNewSession={vi.fn()}
      />
    );
    expect(screen.getByText("Exploring")).toBeTruthy();
    expect(screen.getByText("Agent 2")).toBeTruthy();
  });
});
