import { DownloadCloud, Loader2, X } from "lucide-react";
import { api } from "../lib/bridge";
import { useAppStore } from "../store";
import { cx, formatBytes } from "../lib/format";
import { ProgressBar } from "./Ui";

const PHASE_LABEL: Record<string, string> = {
  resolve: "解析版本清单",
  client: "校验客户端核心",
  libraries: "同步运行库",
  assets: "同步资源对象",
  logging: "同步日志配置",
  loader: "运行加载器安装器",
  done: "完成",
};

export function InstallDock() {
  const install = useAppStore((s) => s.install);
  const contentTask = useAppStore((s) => s.contentTask);
  const cancelInstall = useAppStore((s) => s.cancelInstall);
  if (!install?.running && !contentTask) return null;

  if (!install?.running && contentTask) {
    return (
      <aside className="install-dock is-indeterminate" aria-live="polite">
        <div className="install-dock-head">
          <span className="install-dock-icon">
            <Loader2 size={17} className="spin" />
          </span>
          <div className="install-dock-copy">
            <strong>{contentTask.title}</strong>
            <span>{contentTask.detail}</span>
          </div>
          <button
            type="button"
            className="install-dock-cancel"
            onClick={() => void api.cancelModrinthInstall().catch(() => {})}
            title="取消下载"
          >
            <X size={13} />
            取消
          </button>
        </div>
        <div className="install-dock-progress">
          <ProgressBar value={0} indeterminate />
          <div className="install-dock-meta">
            <span>正在下载</span>
            <span>取消后可以重新下载</span>
          </div>
        </div>
      </aside>
    );
  }

  const progress = install?.progress;
  const percent = progress?.percent ?? 0;
  const loader = install?.loader;
  const title =
    loader === "repair"
      ? "修复检查中"
      : loader === "fabric"
        ? "安装 Fabric"
        : loader === "forge"
          ? "安装 Forge"
          : loader === "neoforge"
            ? "安装 NeoForge"
            : "安装原版";
  const detail = progress
    ? `${PHASE_LABEL[progress.phase] ?? progress.label} · ${progress.current}/${progress.total || "?"}`
    : "正在准备下载管线…";

  return (
    <aside className={cx("install-dock", !progress && "is-indeterminate")} aria-live="polite">
      <div className="install-dock-head">
        <span className="install-dock-icon">
          {progress ? <DownloadCloud size={17} /> : <Loader2 size={17} className="spin" />}
        </span>
        <div className="install-dock-copy">
          <strong>{title}</strong>
          <span>{detail}</span>
        </div>
        <button
          type="button"
          className="install-dock-cancel"
          onClick={() => void cancelInstall()}
          title="取消下载"
        >
          <X size={13} />
          取消
        </button>
      </div>
      <div className="install-dock-progress">
        <ProgressBar value={percent} indeterminate={!progress} />
        <div className="install-dock-meta">
          <span>{progress ? `${percent.toFixed(0)}%` : "正在准备"}</span>
          <span>
            {progress && progress.bytesTotal > 0
              ? `${formatBytes(progress.bytesDone)} / ${formatBytes(progress.bytesTotal)}`
              : "校验完成后开始下载"}
          </span>
        </div>
      </div>
    </aside>
  );
}
