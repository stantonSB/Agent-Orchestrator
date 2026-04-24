import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SessionCard } from "./SessionCard";
import type { SessionInfo } from "../../types/session";

function makeSession(overrides?: Partial<SessionInfo>): SessionInfo {
  return {
    id: "test-1",
    name: "My Session",
    status: "idle",
    createdAt: Date.now(),
    cwd: "/projects/app",
    sessionType: "claude",
    isGitRepo: true,
    ...overrides,
  };
}

describe("SessionCard rename", () => {
  it("enters edit mode on double-click of the name", () => {
    render(
      <SessionCard
        session={makeSession()}
        isActive={false}
        onClick={vi.fn()}
        onRename={vi.fn()}
      />
    );

    const nameEl = screen.getByText("My Session");
    fireEvent.doubleClick(nameEl);

    const input = screen.getByDisplayValue("My Session");
    expect(input).toBeTruthy();
    expect(input.tagName).toBe("INPUT");
  });

  it("saves on Enter and calls onRename", () => {
    const onRename = vi.fn();
    render(
      <SessionCard
        session={makeSession()}
        isActive={false}
        onClick={vi.fn()}
        onRename={onRename}
      />
    );

    fireEvent.doubleClick(screen.getByText("My Session"));
    const input = screen.getByDisplayValue("My Session");
    fireEvent.change(input, { target: { value: "Renamed" } });
    fireEvent.keyDown(input, { key: "Enter" });

    expect(onRename).toHaveBeenCalledWith("test-1", "Renamed");
  });

  it("cancels on Escape without calling onRename", () => {
    const onRename = vi.fn();
    render(
      <SessionCard
        session={makeSession()}
        isActive={false}
        onClick={vi.fn()}
        onRename={onRename}
      />
    );

    fireEvent.doubleClick(screen.getByText("My Session"));
    const input = screen.getByDisplayValue("My Session");
    fireEvent.change(input, { target: { value: "Renamed" } });
    fireEvent.keyDown(input, { key: "Escape" });

    expect(onRename).not.toHaveBeenCalled();
    expect(screen.getByText("My Session")).toBeTruthy();
  });

  it("reverts to original name if input is empty on save", () => {
    const onRename = vi.fn();
    render(
      <SessionCard
        session={makeSession()}
        isActive={false}
        onClick={vi.fn()}
        onRename={onRename}
      />
    );

    fireEvent.doubleClick(screen.getByText("My Session"));
    const input = screen.getByDisplayValue("My Session");
    fireEvent.change(input, { target: { value: "   " } });
    fireEvent.keyDown(input, { key: "Enter" });

    expect(onRename).not.toHaveBeenCalled();
  });

  it("does not call onRename if name is unchanged", () => {
    const onRename = vi.fn();
    render(
      <SessionCard
        session={makeSession()}
        isActive={false}
        onClick={vi.fn()}
        onRename={onRename}
      />
    );

    fireEvent.doubleClick(screen.getByText("My Session"));
    const input = screen.getByDisplayValue("My Session");
    fireEvent.keyDown(input, { key: "Enter" });

    expect(onRename).not.toHaveBeenCalled();
  });

  it("saves on blur and calls onRename", () => {
    const onRename = vi.fn();
    render(
      <SessionCard
        session={makeSession()}
        isActive={false}
        onClick={vi.fn()}
        onRename={onRename}
      />
    );

    fireEvent.doubleClick(screen.getByText("My Session"));
    const input = screen.getByDisplayValue("My Session");
    fireEvent.change(input, { target: { value: "Blurred Name" } });
    fireEvent.blur(input);

    expect(onRename).toHaveBeenCalledWith("test-1", "Blurred Name");
  });

  it("shows Rename option in context menu and enters edit mode on click", () => {
    render(
      <SessionCard
        session={makeSession()}
        isActive={false}
        onClick={vi.fn()}
        onRename={vi.fn()}
      />
    );

    const card = screen.getByText("My Session").closest("[role='button']")!;
    fireEvent.contextMenu(card);

    const renameItem = screen.getByText("Rename");
    expect(renameItem).toBeTruthy();
    fireEvent.click(renameItem);

    expect(screen.getByDisplayValue("My Session")).toBeTruthy();
  });
});

describe("SessionCard terminal sessions", () => {
  it("renders a status dot (not checkmark) for terminal status", () => {
    const session = makeSession({ status: "terminal", sessionType: "terminal" });
    const { container } = render(
      <SessionCard session={session} isActive={false} onClick={vi.fn()} />
    );
    expect(screen.queryByText("✓")).toBeNull();
    expect(container.querySelector('[class*="statusDot"]')).toBeTruthy();
  });

  it("shows 'Terminal' as status label", () => {
    const session = makeSession({ status: "terminal", sessionType: "terminal" });
    render(
      <SessionCard session={session} isActive={false} onClick={vi.fn()} />
    );
    expect(screen.getByText("Terminal")).toBeTruthy();
  });

  it("treats terminal sessions as running (closeable, not dismissable)", () => {
    const onClose = vi.fn();
    const onDismiss = vi.fn();
    const session = makeSession({ status: "terminal", sessionType: "terminal" });
    render(
      <SessionCard
        session={session}
        isActive={false}
        onClick={vi.fn()}
        onClose={onClose}
        onDismiss={onDismiss}
      />
    );

    const card = screen.getByText("My Session").closest("[role='button']")!;
    fireEvent.contextMenu(card);

    expect(screen.getByText("Close Session")).toBeTruthy();
    expect(screen.queryByText("Dismiss")).toBeNull();
  });
});
