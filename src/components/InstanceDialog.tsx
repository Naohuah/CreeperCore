import { useEffect, useState } from "react";
import {
  Archive,
  Box,
  FolderOpen,
  HardDrive,
  Image as ImageIcon,
  Info,
  Loader2,
  Package,
  Play,
  RefreshCw,
  Settings2,
  SlidersHorizontal,
  Sun,
} from "lucide-react";
import { Modal } from "./Modal";
import { useAppStore } from "../store";
import { api } from "../lib/bridge";
import {
  cx,
  errorMessage,
  formatBytes,
  formatRelative,
  loaderLabel,
  shortPath,
  versionTitle,
} from "../lib/format";
import type { BackupInfo, InstanceEntry } from "../lib/types";

type InstanceTab =
  | "overview"
  | "launch"
  | "mods"
  | "saves"
  | "resourcepacks"
  | "shaderpacks"
  | "backups";

const TABS: Array<{
  id: InstanceTab;
  label: string;
  icon: typeof Info;
}> = [
  { id: "overview", label: "概览", icon: Info },
  { id: "launch", label: "启动与内存", icon: SlidersHorizontal },
  { id: "mods", label: "模组", icon: Package },
  { id: "saves", label: "存档", icon: HardDrive },
  { id: "resourcepacks", label: "资源包", icon: ImageIcon },
  { id: "shaderpacks", label: "光影包", icon: Sun },
  { id: "backups", label: "备份", icon: Archive },
];

const CONTENT_KINDS = new Set<InstanceTab>([
  "mods",
  "saves",
  "resourcepacks",
  "shaderpacks",
]);

interface InstanceDialogProps {
  open: boolean;
  versionId: string;
  onClose: () => void;
}

export function InstanceDialog({ open, versionId, onClose }: InstanceDialogProps) {
  const boot = useAppStore((s) => s.boot);
  const refresh = useAppStore((s) => s.refresh);
  const navigate = useAppStore((s) => s.navigate);
  const selectVersion = useAppStore((s) => s.selectVersion);
  const launch = useAppStore((s) => s.launch);
  const toast = useAppStore((s) => s.toast);
  const [tab, setTab] = useState<InstanceTab>("overview");
  const [entries, setEntries] = useState<InstanceEntry[]>([]);
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [followGlobalMemory, setFollowGlobalMemory] = useState(true);
  const [memory, setMemory] = useState("4096");
  const [javaPath, setJavaPath] = useState("");
  const [jvmArgs, setJvmArgs] = useState("");
  const [isolate, setIsolate] = useState<"inherit" | "on" | "off">("inherit");

  const version = boot?.versions.find((item) => item.id === versionId);
  const settings = boot?.config.versionSettings?.[versionId];

  useEffect(() => {
    if (!open) return;
    setTab("overview");
    setError(null);
    setEntries([]);
    setBackups([]);
    const value = boot?.config.versionSettings?.[versionId];
    setFollowGlobalMemory(value?.memoryMb == null);
    setMemory(String(value?.memoryMb ?? boot?.config.memoryMb ?? 4096));
    setJavaPath(value?.javaPath ?? "");
    setJvmArgs(value?.jvmArgs ?? "");
    setIsolate(value?.isolate == null ? "inherit" : value.isolate ? "on" : "off");
  }, [open, versionId]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!open) return;
    if (CONTENT_KINDS.has(tab)) {
      let active = true;
      setLoading(true);
      setError(null);
      void api
        .listInstanceEntries(versionId, tab)
        .then((result) => {
          if (active) setEntries(result);
        })
        .catch((err) => {
          if (active) setError(errorMessage(err));
        })
        .finally(() => {
          if (active) setLoading(false);
        });
      return () => {
        active = false;
      };
    }
    if (tab === "backups") {
      let active = true;
      setLoading(true);
      setError(null);
      void api
        .listBackups(versionId)
        .then((result) => {
          if (active) setBackups(result);
        })
        .catch((err) => {
          if (active) setError(errorMessage(err));
        })
        .finally(() => {
          if (active) setLoading(false);
        });
      return () => {
        active = false;
      };
    }
  }, [open, tab, versionId]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!boot || !version) return null;

  const openFolder = async (target: string) => {
    try {
      await api.openPath(target, versionId);
    } catch (err) {
      toast("error", "无法打开目录", errorMessage(err));
    }
  };

  const openMods = async () => {
    if (boot.config.selectedVersion !== versionId) {
      await selectVersion(versionId);
    }
    navigate("mods");
    onClose();
  };

  const saveSettings = async () => {
    if (saving) return;
    if (!followGlobalMemory) {
      const value = Number(memory);
      if (!Number.isFinite(value) || value < 512) {
        setError("内存至少需要 512 MB");
        return;
      }
    }
    setSaving(true);
    setError(null);
    try {
      await api.saveVersionSettings(versionId, {
        memoryMb: followGlobalMemory ? null : Number(memory),
        javaPath: javaPath.trim() ? javaPath : null,
        jvmArgs: jvmArgs.trim() ? jvmArgs : null,
        isolate: isolate === "inherit" ? null : isolate === "on",
      });
      await refresh(true);
      toast("success", "实例设置已保存");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSaving(false);
    }
  };

  const title = versionTitle(version.id, version.inheritsFrom, version.loader);
  const effectiveJava =
    settings?.javaPath || boot.config.javaPath || "自动选择";
  const effectiveMemory =
    settings?.memoryMb ?? boot.config.memoryMb;
  const effectiveIsolate = settings?.isolate ?? boot.config.isolateVersions;
  const gameDirectory = effectiveIsolate
    ? `${boot.config.gameDir}\\versions\\${versionId}\\game`
    : boot.config.gameDir;

  const renderEntries = () => {
    if (loading) {
      return (
        <div className="instance-empty">
          <Loader2 size={16} className="spin" />
          正在读取…
        </div>
      );
    }
    if (error) {
      return <div className="instance-empty is-error">{error}</div>;
    }
    if (entries.length === 0) {
      return (
        <div className="instance-empty">
          <Box size={22} />
          {tab === "saves"
            ? "还没有存档"
            : tab === "mods"
              ? "还没有安装模组"
              : "目录是空的"}
        </div>
      );
    }
    return (
      <div className="instance-entry-list">
        {entries.map((entry) => (
          <div className="instance-entry" key={entry.path}>
            <span className="instance-entry-icon">
              {entry.isDir ? <FolderOpen size={15} /> : <Box size={15} />}
            </span>
            <div>
              <strong>{entry.name}</strong>
              <span>
                {entry.isDir ? "文件夹" : formatBytes(entry.size)}
                {entry.modified ? ` · ${formatRelative(entry.modified)}` : ""}
              </span>
            </div>
          </div>
        ))}
      </div>
    );
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="版本设置"
      description={`${title} · ${loaderLabel(version.loader)}`}
      size="lg"
      className="instance-modal"
    >
      <div className="instance-layout">
        <nav className="instance-nav">
          {TABS.map((item) => {
            const Icon = item.icon;
            return (
              <button
                type="button"
                key={item.id}
                className={cx(tab === item.id && "is-active")}
                onClick={() => setTab(item.id)}
              >
                <Icon size={14} />
                {item.label}
              </button>
            );
          })}
        </nav>

        <div className="instance-body">
          {tab === "overview" && (
            <div className="instance-section">
              <div className="instance-title-row">
                <span className={cx("version-avatar", `is-${version.loader}`)}>
                  {loaderLabel(version.loader).slice(0, 1)}
                </span>
                <div>
                  <strong>{title}</strong>
                  <span>{version.id}</span>
                </div>
              </div>
              <div className="instance-rows">
                <div>
                  <span>游戏目录</span>
                  <code>{shortPath(gameDirectory, 62)}</code>
                </div>
                <div>
                  <span>Java</span>
                  <code>{shortPath(effectiveJava, 62)}</code>
                </div>
                <div>
                  <span>内存</span>
                  <span>{effectiveMemory} MB</span>
                </div>
                <div>
                  <span>版本隔离</span>
                  <span>{effectiveIsolate ? "独立目录" : "共用游戏目录"}</span>
                </div>
              </div>
              <div className="instance-actions">
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => void openFolder("game")}
                >
                  <FolderOpen size={14} />
                  打开游戏目录
                </button>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => void openFolder("version")}
                >
                  <Box size={14} />
                  打开版本目录
                </button>
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={() => void launch(versionId)}
                >
                  <Play size={14} fill="currentColor" />
                  启动这个版本
                </button>
              </div>
            </div>
          )}

          {tab === "launch" && (
            <div className="instance-section">
              <div className="instance-section-head">
                <strong>启动与内存</strong>
                <span>留空或选择“跟随全局”时使用全局设置</span>
              </div>
              <label className="instance-field">
                <span>内存</span>
                <div className="instance-memory-row">
                  <label className="check-label">
                    <input
                      type="checkbox"
                      checked={followGlobalMemory}
                      onChange={(event) => setFollowGlobalMemory(event.target.checked)}
                    />
                    跟随全局（{boot.config.memoryMb} MB）
                  </label>
                  <input
                    type="number"
                    min={512}
                    max={32768}
                    value={memory}
                    disabled={followGlobalMemory}
                    onChange={(event) => setMemory(event.target.value)}
                  />
                  <span>MB</span>
                </div>
              </label>
              <label className="instance-field">
                <span>Java</span>
                <select
                  value={javaPath}
                  onChange={(event) => setJavaPath(event.target.value)}
                >
                  <option value="">
                    跟随全局（
                    {boot.selectedJava
                      ? `Java ${boot.selectedJava.major}`
                      : "自动选择"}
                    ）
                  </option>
                  {boot.javas.map((java) => (
                    <option key={java.path} value={java.path}>
                      Java {java.major} · {java.vendor}
                    </option>
                  ))}
                </select>
              </label>
              <label className="instance-field">
                <span>JVM 参数</span>
                <textarea
                  className="code-input"
                  rows={3}
                  value={jvmArgs}
                  placeholder={boot.config.jvmArgs || "留空时使用全局 JVM 参数"}
                  onChange={(event) => setJvmArgs(event.target.value)}
                />
              </label>
              <label className="instance-field">
                <span>版本隔离</span>
                <select
                  value={isolate}
                  onChange={(event) =>
                    setIsolate(event.target.value as "inherit" | "on" | "off")
                  }
                >
                  <option value="inherit">
                    跟随全局（{boot.config.isolateVersions ? "开启" : "关闭"}）
                  </option>
                  <option value="on">开启</option>
                  <option value="off">关闭</option>
                </select>
              </label>
              {error && <p className="form-error">{error}</p>}
              <div className="instance-actions">
                <button
                  type="button"
                  className="btn btn-primary"
                  disabled={saving}
                  onClick={() => void saveSettings()}
                >
                  {saving ? <Loader2 size={14} className="spin" /> : <Settings2 size={14} />}
                  保存实例设置
                </button>
                <button
                  type="button"
                  className="btn btn-ghost"
                  onClick={() => void refresh(true)}
                >
                  <RefreshCw size={14} />
                  重新读取
                </button>
              </div>
            </div>
          )}

          {CONTENT_KINDS.has(tab) && (
            <div className="instance-section">
              <div className="instance-section-head">
                <strong>{TABS.find((item) => item.id === tab)?.label}</strong>
                <div className="instance-section-actions">
                  {tab === "mods" && (
                    <button type="button" className="text-button" onClick={() => void openMods()}>
                      打开模组管理
                    </button>
                  )}
                  <button
                    type="button"
                    className="text-button"
                    onClick={() => void openFolder(tab)}
                  >
                    <FolderOpen size={13} />
                    打开文件夹
                  </button>
                </div>
              </div>
              {renderEntries()}
            </div>
          )}

          {tab === "backups" && (
            <div className="instance-section">
              <div className="instance-section-head">
                <strong>备份</strong>
                <button
                  type="button"
                  className="text-button"
                  onClick={() => {
                    navigate("backups");
                    onClose();
                  }}
                >
                  打开备份管理
                </button>
              </div>
              {loading ? (
                <div className="instance-empty">
                  <Loader2 size={16} className="spin" />
                  正在读取…
                </div>
              ) : backups.length === 0 ? (
                <div className="instance-empty">
                  <Archive size={22} />
                  还没有这个版本的备份
                </div>
              ) : (
                <div className="instance-entry-list">
                  {backups.map((backup) => (
                    <div className="instance-entry" key={backup.id}>
                      <span className="instance-entry-icon">
                        <Archive size={15} />
                      </span>
                      <div>
                        <strong>{backup.name}</strong>
                        <span>
                          {backup.kind === "full" ? "完整备份" : "存档备份"} ·{" "}
                          {formatRelative(backup.createdAt)} ·{" "}
                          {formatBytes(backup.sizeBytes)}
                        </span>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </Modal>
  );
}
