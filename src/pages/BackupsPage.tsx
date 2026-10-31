import { useCallback, useEffect, useState } from "react";
import {
  Archive,
  Clock3,
  FolderOpen,
  Plus,
  RefreshCw,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { useAppStore } from "../store";
import { api } from "../lib/bridge";
import {
  errorMessage,
  formatBytes,
  formatDateTime,
  loaderLabel,
} from "../lib/format";
import { CreeperMark } from "../components/CreeperMark";
import { EmptyState, LoaderBadge, PageHeader, Toggle } from "../components/Ui";
import { Modal } from "../components/Modal";
import type { BackupInfo } from "../lib/types";

export function BackupsPage() {
  const boot = useAppStore((s) => s.boot);
  const navigate = useAppStore((s) => s.navigate);
  const toast = useAppStore((s) => s.toast);
  const askConfirm = useAppStore((s) => s.askConfirm);
  const refresh = useAppStore((s) => s.refresh);
  const selectedId = boot?.config.selectedVersion ?? null;
  const version = boot?.versions.find((item) => item.id === selectedId);
  const [backups, setBackups] = useState<BackupInfo[]>(boot?.backups ?? []);
  const [modalOpen, setModalOpen] = useState(false);
  const [name, setName] = useState("");
  const [note, setNote] = useState("");
  const [includeMods, setIncludeMods] = useState(false);
  const [creating, setCreating] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);

  useEffect(() => {
    setBackups(boot?.backups ?? []);
  }, [boot?.backups]);

  const loadBackups = useCallback(async () => {
    if (!selectedId) return;
    try {
      setBackups(await api.listBackups(selectedId));
    } catch (error) {
      toast("error", "无法读取备份列表", errorMessage(error));
    }
  }, [selectedId, toast]);

  useEffect(() => {
    if (selectedId) void loadBackups();
  }, [selectedId, loadBackups]);

  const totalSize = backups.reduce((sum, item) => sum + item.sizeBytes, 0);

  const openBackupFolder = async () => {
    try {
      await api.openPath("backups", selectedId);
    } catch (error) {
      toast("error", "无法打开备份目录", errorMessage(error));
    }
  };

  const create = async () => {
    if (!selectedId || creating) return;
    setCreating(true);
    try {
      const backup = await api.createBackup(
        selectedId,
        name.trim() || "手动备份",
        note.trim(),
        includeMods,
      );
      setBackups((items) => [backup, ...items]);
      setModalOpen(false);
      setName("");
      setNote("");
      setIncludeMods(false);
      toast("success", "备份已创建", `${backup.fileCount} 个文件 · ${formatBytes(backup.sizeBytes)}`);
      await refresh(true);
    } catch (error) {
      toast("error", "创建备份失败", errorMessage(error));
    } finally {
      setCreating(false);
    }
  };

  const restore = async (backup: BackupInfo) => {
    if (!selectedId) return;
    const accepted = await askConfirm({
      title: `恢复备份「${backup.name}」？`,
      description:
        "当前存档与配置会被备份覆盖。启动器会先自动创建一份恢复前备份，仍然可以再退回来。",
      confirmLabel: "开始恢复",
      danger: true,
    });
    if (!accepted) return;
    setBusyId(backup.id);
    try {
      const report = await api.restoreBackup(selectedId, backup.id);
      toast("success", "恢复完成", report.message);
      await refresh(true);
      await loadBackups();
    } catch (error) {
      toast("error", "恢复失败", errorMessage(error));
    } finally {
      setBusyId(null);
    }
  };

  const remove = async (backup: BackupInfo) => {
    if (!selectedId) return;
    const accepted = await askConfirm({
      title: `删除备份「${backup.name}」？`,
      description: "备份文件会永久删除。恢复前自动创建的备份不会受影响。",
      confirmLabel: "永久删除",
      danger: true,
    });
    if (!accepted) return;
    setBusyId(backup.id);
    try {
      await api.deleteBackup(selectedId, backup.id);
      setBackups((items) => items.filter((item) => item.id !== backup.id));
      toast("success", "备份已删除");
    } catch (error) {
      toast("error", "删除失败", errorMessage(error));
    } finally {
      setBusyId(null);
    }
  };

  if (!boot) return null;

  return (
    <div className="page backups-page">
      <PageHeader
        title="备份与恢复"
        description="为当前版本的存档与配置创建快照，随时恢复。"
        actions={
          <div className="page-action-row">
            <button type="button" className="btn btn-ghost" onClick={() => void openBackupFolder()}>
              <FolderOpen size={15} />
              打开目录
            </button>
            <button type="button" className="btn btn-ghost" onClick={() => void loadBackups()}>
              <RefreshCw size={15} />
              刷新
            </button>
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => setModalOpen(true)}
              disabled={!selectedId}
            >
              <Plus size={15} />
              创建备份
            </button>
          </div>
        }
      />

      {!selectedId ? (
        <section className="panel">
          <EmptyState
            art={<CreeperMark size={56} />}
            title="先选择一个版本"
            description="备份属于具体的游戏版本。选择一个版本后再创建或恢复备份。"
            action={
              <button type="button" className="btn btn-primary" onClick={() => navigate("versions")}>
                前往版本管理
              </button>
            }
          />
        </section>
      ) : (
        <>
          <div className="context-bar">
            <div className="context-main">
              <CreeperMark size={28} />
              <strong>{version?.id ?? selectedId}</strong>
              <LoaderBadge loader={version?.loader ?? "vanilla"} compact />
              <span>{loaderLabel(version?.loader)}</span>
            </div>
            <div className="context-stats">
              <span>{backups.length} 个备份</span>
              <span>{formatBytes(totalSize)}</span>
            </div>
          </div>

          {backups.length === 0 ? (
            <section className="panel">
              <EmptyState
                art={<Clock3 size={30} />}
                title="还没有备份"
                description="创建一次快照。恢复前也会自动保存当前状态。"
                action={
                  <button type="button" className="btn btn-primary" onClick={() => setModalOpen(true)}>
                    创建第一个备份
                  </button>
                }
              />
            </section>
          ) : (
            <section className="panel data-panel">
              <div className="data-head backup-table">
                <span>名称</span>
                <span>类型</span>
                <span>创建时间</span>
                <span>大小</span>
                <span>操作</span>
              </div>
              {backups.map((backup) => (
                <div className="data-row backup-table" key={backup.id}>
                  <div className="backup-name">
                    <Archive size={16} />
                    <div>
                      <strong>{backup.name}</strong>
                      {backup.note && <span>{backup.note}</span>}
                    </div>
                  </div>
                  <span className={backup.kind === "full" ? "tag tag-blue" : "tag tag-green"}>
                    {backup.kind === "full" ? "完整" : "存档"}
                  </span>
                  <span className="cell-muted">{formatDateTime(backup.createdAt)}</span>
                  <span className="cell-muted">{formatBytes(backup.sizeBytes)}</span>
                  <div className="row-actions">
                    <button
                      type="button"
                      className="btn btn-sm btn-secondary"
                      disabled={busyId !== null}
                      onClick={() => void restore(backup)}
                    >
                      {busyId === backup.id ? "恢复中…" : "恢复"}
                    </button>
                    <button
                      type="button"
                      className="icon-button subtle danger-hover"
                      disabled={busyId !== null}
                      aria-label={`删除备份 ${backup.name}`}
                      onClick={() => void remove(backup)}
                    >
                      <Trash2 size={14} />
                    </button>
                  </div>
                </div>
              ))}
            </section>
          )}

          <div className="info-line">
            <ShieldCheck size={14} />
            恢复前会自动创建安全备份，避免误操作丢失存档。
          </div>
        </>
      )}

      <Modal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        title="创建备份"
        description="备份范围可以只包含存档，也可以包含模组和配置。"
        footer={
          <>
            <button type="button" className="btn btn-ghost" onClick={() => setModalOpen(false)}>
              取消
            </button>
            <button type="button" className="btn btn-primary" disabled={creating} onClick={() => void create()}>
              {creating ? "正在创建…" : "创建备份"}
            </button>
          </>
        }
      >
        <div className="form-stack">
          <label className="field">
            <span>备份名称</span>
            <input
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="例如：打末影龙之前"
              maxLength={64}
            />
          </label>
          <label className="field">
            <span>备注（可选）</span>
            <textarea
              value={note}
              onChange={(event) => setNote(event.target.value)}
              placeholder="写一句留给未来的话…"
              rows={3}
              maxLength={240}
            />
          </label>
          <div className="choice-row">
            <div>
              <strong>包含模组与配置</strong>
              <span>额外保存 mods、config 和资源包，体积会更大。</span>
            </div>
            <Toggle checked={includeMods} onChange={setIncludeMods} label="包含模组与配置" />
          </div>
          <p className="field-hint">
            {includeMods
              ? "保存 saves、mods、config 与 resourcepacks。"
              : "保存 saves、options.txt 与 servers.dat。"}
          </p>
        </div>
      </Modal>
    </div>
  );
}
