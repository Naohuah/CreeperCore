import {
  AlertCircle,
  AlertTriangle,
  Check,
  CheckCircle2,
  Info,
  RefreshCw,
  Wrench,
} from "lucide-react";
import { useAppStore } from "../store";
import { Modal } from "./Modal";
import { cx } from "../lib/format";

interface DiagnosticsDialogProps {
  open: boolean;
  onClose: () => void;
  onRepair: () => void;
}

const ICONS = {
  ok: Check,
  info: Info,
  warn: AlertTriangle,
  error: AlertCircle,
};

export function DiagnosticsDialog({
  open,
  onClose,
  onRepair,
}: DiagnosticsDialogProps) {
  const boot = useAppStore((s) => s.boot);
  const refresh = useAppStore((s) => s.refresh);
  const navigate = useAppStore((s) => s.navigate);
  const report = boot?.diagnostics;
  const issueCount =
    report?.items.filter(
      (item) => item.severity === "warn" || item.severity === "error",
    ).length ?? 0;
  const tone =
    report?.level === "danger" || report?.hasMissingFiles
      ? "danger"
      : issueCount > 0
        ? "attention"
        : "ready";
  const SummaryIcon =
    tone === "danger" ? AlertCircle : tone === "attention" ? AlertTriangle : CheckCircle2;

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="运行检查"
      description="检查 Java、核心文件、资源和当前版本的模组状态。"
      size="md"
      footer={
        <>
          <button
            type="button"
            className="btn btn-ghost"
            onClick={() => {
              void refresh(true);
            }}
          >
            <RefreshCw size={14} />
            重新检查
          </button>
          {report?.hasMissingFiles ? (
            <button type="button" className="btn btn-primary" onClick={onRepair}>
              <Wrench size={14} />
              修复核心文件
            </button>
          ) : (
            <button type="button" className="btn btn-secondary" onClick={onClose}>
              完成
            </button>
          )}
        </>
      }
    >
      {!report ? (
        <div className="diagnostics-empty">正在读取运行环境…</div>
      ) : (
        <div className="diagnostics">
          <div className={cx("diagnostics-summary", `is-${tone}`)}>
            <span className="diagnostics-score">{report.score}</span>
            <div>
              <strong>
                {tone === "attention"
                  ? `可以启动，有 ${issueCount} 项提示`
                  : report.headline}
              </strong>
              <span>满分 100 · 所有检查均在本地完成</span>
            </div>
            <SummaryIcon size={22} />
          </div>

          <div className="diagnostics-list">
            {report.items.map((item) => {
              const Icon = ICONS[item.severity as keyof typeof ICONS] ?? Info;
              return (
                <div className={cx("diagnostic-row", `is-${item.severity}`)} key={item.id}>
                  <span className="diagnostic-icon">
                    <Icon size={15} />
                  </span>
                  <div>
                    <strong>{item.title}</strong>
                    <p>{item.detail}</p>
                  </div>
                  {item.fixable && <span className="diagnostic-fixable">可修复</span>}
                </div>
              );
            })}
          </div>

          {report.items.some((item) => item.action === "mods") && (
            <button
              type="button"
              className="text-button diagnostics-link"
              onClick={() => {
                onClose();
                navigate("mods");
              }}
            >
              前往模组管理
            </button>
          )}
        </div>
      )}
    </Modal>
  );
}
