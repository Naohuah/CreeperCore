import { Check, Download, Layers } from "lucide-react";
import { Modal } from "./Modal";
import { useAppStore } from "../store";
import { cx, loaderLabel, versionTitle } from "../lib/format";

interface VersionSelectDialogProps {
  open: boolean;
  onClose: () => void;
}

export function VersionSelectDialog({ open, onClose }: VersionSelectDialogProps) {
  const boot = useAppStore((s) => s.boot);
  const navigate = useAppStore((s) => s.navigate);
  const selectVersion = useAppStore((s) => s.selectVersion);
  const selectedId = boot?.config.selectedVersion;
  const versions = [...(boot?.versions ?? [])].sort(
    (a, b) => b.lastModified - a.lastModified,
  );

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="版本选择"
      description="选中的版本会成为启动按钮使用的版本"
      size="sm"
    >
      <div className="version-picker">
        {versions.length === 0 ? (
          <div className="version-picker-empty">
            <Layers size={24} />
            <span>还没有安装任何版本</span>
          </div>
        ) : (
          <div className="version-picker-list">
            {versions.map((version) => {
              const active = version.id === selectedId;
              return (
                <button
                  type="button"
                  key={version.id}
                  className={cx("version-picker-row", active && "is-active")}
                  onClick={() => {
                    if (!active) void selectVersion(version.id);
                    onClose();
                  }}
                >
                  <span className={cx("version-avatar", `is-${version.loader}`)}>
                    {loaderLabel(version.loader).slice(0, 1)}
                  </span>
                  <span className="version-picker-copy">
                    <strong>
                      {versionTitle(version.id, version.inheritsFrom, version.loader)}
                    </strong>
                    <small>
                      {loaderLabel(version.loader)}
                      {version.javaMajor ? ` · Java ${version.javaMajor}` : ""}
                      {!version.hasClient ? " · 文件不全" : ""}
                    </small>
                  </span>
                  {active && <Check size={14} />}
                </button>
              );
            })}
          </div>
        )}
        <div className="version-picker-actions">
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => {
              onClose();
              navigate("downloads");
            }}
          >
            <Download size={14} />
            安装新版本
          </button>
          <button
            type="button"
            className="btn btn-ghost"
            onClick={() => {
              onClose();
              navigate("versions");
            }}
          >
            版本管理
          </button>
        </div>
      </div>
    </Modal>
  );
}
