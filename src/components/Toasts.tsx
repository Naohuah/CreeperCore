import {
  AlertTriangle,
  CheckCircle2,
  Info,
  ShieldAlert,
  X,
  XCircle,
} from "lucide-react";
import { useAppStore } from "../store";
import { cx } from "../lib/format";

const ICONS = {
  info: Info,
  success: CheckCircle2,
  warning: AlertTriangle,
  error: XCircle,
};

export function Toasts() {
  const toasts = useAppStore((s) => s.toasts);
  const dismiss = useAppStore((s) => s.dismissToast);
  return (
    <div className="toast-stack" aria-live="polite" aria-atomic="false">
      {toasts.map((toast) => {
        const Icon = ICONS[toast.kind] ?? ShieldAlert;
        return (
          <article key={toast.id} className={cx("toast", `toast-${toast.kind}`)}>
            <span className="toast-icon">
              <Icon size={17} />
            </span>
            <div className="toast-copy">
              <strong>{toast.title}</strong>
              {toast.detail && <span>{toast.detail}</span>}
            </div>
            <button
              type="button"
              className="toast-close"
              onClick={() => dismiss(toast.id)}
              aria-label="关闭通知"
            >
              <X size={14} />
            </button>
          </article>
        );
      })}
    </div>
  );
}
