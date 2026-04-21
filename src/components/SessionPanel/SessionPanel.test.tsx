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
      { id: "1", name: "Session A", status: "working", createdAt: Date.now(), cwd: "/projects/app" },
      { id: "2", name: "Session B", status: "idle", createdAt: Date.now(), cwd: "/projects/app" },
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
      { id: "1", name: "Fix bug", status: "working", createdAt: 1000, cwd: "/projects/app-one" },
      { id: "2", name: "Add feature", status: "idle", createdAt: 2000, cwd: "/projects/app-two" },
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
      { id: "1", name: "Fix bug", status: "working", createdAt: 1000, cwd: "/projects/app-one" },
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
      { id: "1", name: "Old session", status: "idle", createdAt: 1000, cwd: "/projects/old-project" },
      { id: "2", name: "New session", status: "working", createdAt: 3000, cwd: "/projects/new-project" },
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
      { id: "1", name: "Work app", status: "working", createdAt: 1000, cwd: "/work/myapp" },
      { id: "2", name: "Personal app", status: "idle", createdAt: 2000, cwd: "/personal/myapp" },
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
});
