import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { SubagentList } from "./SubagentList";
import type { SubagentStatus } from "../../types/session";

describe("SubagentList", () => {
  it("renders nothing when subagents is empty", () => {
    const { container } = render(<SubagentList subagents={[]} />);
    expect(container.innerHTML).toBe("");
  });

  it("renders a dot and name for each subagent", () => {
    const subagents: SubagentStatus[] = [
      { id: "a", index: 1, status: "working", name: "Exploring codebase" },
      { id: "b", index: 2, status: "idle", name: null },
    ];
    render(<SubagentList subagents={subagents} />);
    expect(screen.getByText("Exploring codebase")).toBeTruthy();
    expect(screen.getByText("Agent 2")).toBeTruthy();
  });

  it("renders finished subagents with dimmed class", () => {
    const subagents: SubagentStatus[] = [
      { id: "a", index: 1, status: "finished", name: "Done agent" },
    ];
    const { container } = render(<SubagentList subagents={subagents} />);
    const entry = container.querySelector("[class*='finished']");
    expect(entry).toBeTruthy();
  });

  it("shows correct status dot classes", () => {
    const subagents: SubagentStatus[] = [
      { id: "a", index: 1, status: "needs_attention", name: null },
    ];
    const { container } = render(<SubagentList subagents={subagents} />);
    const dot = container.querySelector("[class*='NeedsAttention']");
    expect(dot).toBeTruthy();
  });
});
