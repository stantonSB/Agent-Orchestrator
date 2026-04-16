import { useSessionStore } from "../../stores/sessionStore";
import { Toast } from "../Toast/Toast";

const containerStyle: React.CSSProperties = {
  position: "fixed",
  top: 48,
  right: 16,
  display: "flex",
  flexDirection: "column",
  gap: 8,
  zIndex: 3000,
  pointerEvents: "none",
};

const itemStyle: React.CSSProperties = {
  pointerEvents: "auto",
};

export function ToastContainer() {
  const toasts = useSessionStore((s) => s.toasts);
  const dismissToast = useSessionStore((s) => s.dismissToast);

  return (
    <div style={containerStyle}>
      {toasts.map((toast) => (
        <div key={toast.id} style={itemStyle}>
          <Toast toast={toast} onDismiss={dismissToast} />
        </div>
      ))}
    </div>
  );
}
