import { useEffect } from "react";

interface GlobalKeybindingActions {
  onNewSession: () => void;
  onCloseActiveSession: () => void;
}

export function useGlobalKeybindings({ onNewSession, onCloseActiveSession }: GlobalKeybindingActions) {
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
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onNewSession, onCloseActiveSession]);
}
