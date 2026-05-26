import { useEffect } from "react";

export function getCycledIndex(
  direction: "prev" | "next",
  currentId: string | null,
  orderedIds: string[],
): number | null {
  if (orderedIds.length <= 1 || !currentId) return null;
  const idx = orderedIds.indexOf(currentId);
  if (idx === -1) return null;
  if (direction === "prev") {
    return idx <= 0 ? orderedIds.length - 1 : idx - 1;
  }
  return idx >= orderedIds.length - 1 ? 0 : idx + 1;
}

interface GlobalKeybindingActions {
  onNewSession: () => void;
  onCloseActiveSession: () => void;
  onSwitchToSession: (index: number) => void;
  onCyclePrev: () => void;
  onCycleNext: () => void;
  onOpenSettings: () => void;
}

export function useGlobalKeybindings({
  onNewSession,
  onCloseActiveSession,
  onSwitchToSession,
  onCyclePrev,
  onCycleNext,
  onOpenSettings,
}: GlobalKeybindingActions) {
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (!e.metaKey) return;

      if (e.key === "t") {
        e.preventDefault();
        onNewSession();
      }

      if (e.key === "w") {
        e.preventDefault();
        onCloseActiveSession();
      }

      if (e.key === "," && !e.shiftKey) {
        e.preventDefault();
        onOpenSettings();
      }

      if (e.shiftKey && e.code === "BracketLeft") {
        e.preventDefault();
        onCyclePrev();
      }

      if (e.shiftKey && e.code === "BracketRight") {
        e.preventDefault();
        onCycleNext();
      }

      const digit = parseInt(e.key, 10);
      if (digit >= 1 && digit <= 9) {
        e.preventDefault();
        onSwitchToSession(digit - 1);
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onNewSession, onCloseActiveSession, onSwitchToSession, onCyclePrev, onCycleNext, onOpenSettings]);
}
