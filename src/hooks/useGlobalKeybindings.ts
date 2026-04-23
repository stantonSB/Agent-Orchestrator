import { useEffect } from "react";

interface GlobalKeybindingActions {
  onNewSession: () => void;
  onCloseActiveSession: () => void;
  onSwitchToSession: (index: number) => void;
}

export function useGlobalKeybindings({ onNewSession, onCloseActiveSession, onSwitchToSession }: GlobalKeybindingActions) {
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

      const digit = parseInt(e.key, 10);
      if (digit >= 1 && digit <= 9) {
        e.preventDefault();
        onSwitchToSession(digit - 1);
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onNewSession, onCloseActiveSession, onSwitchToSession]);
}
