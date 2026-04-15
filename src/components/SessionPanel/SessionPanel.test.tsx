import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
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
      { id: "1", name: "Session A", status: "working", createdAt: Date.now() },
      { id: "2", name: "Session B", status: "idle", createdAt: Date.now() },
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
});
