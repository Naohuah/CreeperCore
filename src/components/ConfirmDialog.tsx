import { AlertTriangle } from "lucide-react";
import { useAppStore } from "../store";
import { cx } from "../lib/format";
import { Modal } from "./Modal";

export function ConfirmDialog() {
  const dialog = useAppStore((s) => s.confirmDialog);
  const finish = useAppStore((s) => s.finishConfirm);

  return (
    <Modal
      open={Boolean(dialog)}
      onClose={() => finish(false)}
      size="sm"
      className="confirm-modal"
      dismissible
    >
      {dialog && (
        <div className="confirm-content">
          <span className={cx("confirm-icon", dialog.danger && "is-danger")}>
            <AlertTriangle size={22} />
          </span>
          <div>
            <h2>{dialog.title}</h2>
            <p>{dialog.description}</p>
          </div>
          <div className="confirm-actions">
            <button
              type="button"
              className="btn btn-ghost"
              onClick={() => finish(false)}
            >
              {dialog.cancelLabel ?? "取消"}
            </button>
            <button
              type="button"
              className={cx("btn", dialog.danger ? "btn-danger" : "btn-primary")}
              onClick={() => finish(true)}
              autoFocus
            >
              {dialog.confirmLabel ?? "确认"}
            </button>
          </div>
        </div>
      )}
    </Modal>
  );
}
