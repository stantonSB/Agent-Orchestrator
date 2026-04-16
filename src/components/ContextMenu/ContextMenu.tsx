import { useEffect, useRef } from "react";
import styles from "./ContextMenu.module.css";

interface ContextMenuItem {
  label: string;
  onClick: () => void;
  danger?: boolean;
  disabled?: boolean;
}

interface ContextMenuProps {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}

export function ContextMenu({ x, y, items, onClose }: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    }
    function handleEscape(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("mousedown", handleClickOutside);
    document.addEventListener("keydown", handleEscape);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleEscape);
    };
  }, [onClose]);

  return (
    <div ref={menuRef} className={styles.menu} style={{ left: `${x}px`, top: `${y}px` }}>
      {items.map((item, i) => (
        <button
          key={i}
          className={`${styles.item} ${item.danger ? styles.danger : ""}`}
          onClick={() => { item.onClick(); onClose(); }}
          disabled={item.disabled}
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}
