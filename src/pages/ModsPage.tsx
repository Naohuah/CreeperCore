import { useCallback, useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  DownloadCloud,
  FolderOpen,
  Package,
  Plus,
  RefreshCw,
  Search,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { useAppStore } from "../store";
import { api, isTauri, pickModFiles } from "../lib/bridge";
import {
  cx,
  errorMessage,
  formatBytes,
  formatRelative,
  loaderLabel,
  modColor,
  modInitial,
} from "../lib/format";
import { CreeperMark } from "../components/CreeperMark";
import { EmptyState, LoaderBadge, PageHeader, Toggle } from "../components/Ui";
import type { ModInfo } from "../lib/types";

type ModFilter = "all" | "enabled" | "disabled" | "issues";

export function ModsPage() {
  const boot = useAppStore((s) => s.boot);
  const navigate = useAppStore((s) => s.navigate);
  const openDownloads = useAppStore((s) => s.openDownloads);
  const toast = useAppStore((s) => s.toast);
  const askConfirm = useAppStore((s) => s.askConfirm);
  const selectedId = boot?.config.selectedVersion ?? null;
  const version = boot?.versions.find((item) => item.id === selectedId);
  const [mods, setMods] = useState<ModInfo[]>(boot?.mods ?? []);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<ModFilter>("all");
  const [busyFile, setBusyFile] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);

  useEffect(() => {
    setMods(boot?.mods ?? []);
  }, [boot?.mods]);

  const loadMods = useCallback(async () => {
    if (!selectedId) return;
    try {
      setMods(await api.listMods(selectedId));
    } catch (error) {
      toast("error", "无法读取模组目录", errorMessage(error));
    }
  }, [selectedId, toast]);

  useEffect(() => {
    if (selectedId) void loadMods();
  }, [selectedId, loadMods]);

  const enabled = mods.filter((mod) => mod.enabled);
  const issueMods = mods.filter(
    (mod) => mod.enabled && (mod.missingDependencies.length > 0 || mod.duplicate || mod.readError),
  );
  const disabledCount = mods.length - enabled.length;
  const totalSize = mods.reduce((sum, mod) => sum + mod.size, 0);

  const filtered = useMemo(() => {
    return mods.filter((mod) => {
      if (
        query.trim() &&
        !`${mod.name} ${mod.modId ?? ""} ${mod.fileName}`
          .toLowerCase()
          .includes(query.trim().toLowerCase())
      ) {
        return false;
      }
      if (filter === "enabled") return mod.enabled;
      if (filter === "disabled") return !mod.enabled;
      if (filter === "issues")
        return mod.missingDependencies.length > 0 || mod.duplicate || Boolean(mod.readError);
      return true;
    });
  }, [mods, query, filter]);

  const openModsFolder = async () => {
    try {
      await api.openPath("mods", selectedId);
    } catch (error) {
      toast("error", "无法打开模组目录", errorMessage(error));
    }
  };

  const toggle = async (mod: ModInfo, next: boolean) => {
    if (!selectedId) return;
    setBusyFile(mod.fileName);
    try {
      await api.toggleMod(selectedId, mod.fileName, next);
      await loadMods();
      toast("success", next ? "已启用" : "已禁用", mod.name);
    } catch (error) {
      toast("error", "无法修改模组", errorMessage(error));
    } finally {
      setBusyFile(null);
    }
  };

  const remove = async (mod: ModInfo) => {
    if (!selectedId) return;
    const accepted = await askConfirm({
      title: `删除模组 ${mod.name}？`,
      description: "模组文件会被永久删除，无法恢复。",
      confirmLabel: "永久删除",
      danger: true,
    });
    if (!accepted) return;
    setBusyFile(mod.fileName);
    try {
      await api.deleteMod(selectedId, mod.fileName);
      await loadMods();
      toast("success", "模组已删除", mod.name);
    } catch (error) {
      toast("error", "无法删除模组", errorMessage(error));
    } finally {
      setBusyFile(null);
    }
  };

  const importMods = async () => {
    if (!selectedId) return;
    if (!isTauri()) {
      toast("info", "浏览器预览模式", "文件选择器只在桌面版中可用");
      return;
    }
    setImporting(true);
    try {
      const files = await pickModFiles();
      if (files.length === 0) return;
      const report = await api.importMods(selectedId, files);
      await loadMods();
      if (report.errors.length > 0) {
        toast("error", "部分模组未导入", report.errors.join("；"));
      } else if (report.skipped.length > 0) {
        toast("warning", "已跳过重复文件", report.skipped.join("；"));
      } else {
        toast("success", "安装完成", `${report.changed.length} 个模组已放入 mods 目录`);
      }
    } catch (error) {
      toast("error", "导入失败", errorMessage(error));
    } finally {
      setImporting(false);
    }
  };

  if (!boot) return null;

  return (
    <div className="page mods-page">
      <PageHeader
        title="模组管理"
        description="管理当前版本的模组、依赖和启用状态。"
        actions={
          <div className="page-action-row">
            <button type="button" className="btn btn-ghost" onClick={() => void openModsFolder()}>
              <FolderOpen size={15} />
              打开目录
            </button>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => void loadMods()}
              disabled={busyFile !== null}
            >
              <RefreshCw size={15} />
              刷新
            </button>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => openDownloads("mods")}
            >
              <DownloadCloud size={15} />
              下载新 Mod
            </button>
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => void importMods()}
              disabled={!selectedId || importing || busyFile !== null}
            >
              {importing ? <RefreshCw size={15} className="spin" /> : <Plus size={15} />}
              安装模组
            </button>
          </div>
        }
      />

      {!selectedId ? (
        <section className="panel">
          <EmptyState
            art={<CreeperMark size={56} />}
            title="先选择一个版本"
            description="模组属于具体的游戏版本。安装或选择一个带模组加载器的版本后再来。"
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
              <strong>{version ? version.id : selectedId}</strong>
              <LoaderBadge loader={version?.loader ?? "vanilla"} compact />
              <span>{loaderLabel(version?.loader)}</span>
            </div>
            <div className="context-stats">
              <span>已启用 {enabled.length}</span>
              <span>已禁用 {disabledCount}</span>
              <span>{formatBytes(totalSize)}</span>
            </div>
          </div>

          {issueMods.length > 0 && (
            <div className="inline-alert warn">
              <AlertTriangle size={16} />
              <div>
                <strong>{issueMods.length} 个模组需要关注</strong>
                <span>
                  {issueMods
                    .slice(0, 3)
                    .map((mod) =>
                      mod.missingDependencies.length > 0
                        ? `${mod.name} 缺少 ${mod.missingDependencies.join("、")}`
                        : `${mod.name} 存在重复或读取问题`,
                    )
                    .join("；")}
                </span>
              </div>
              <button type="button" className="text-button" onClick={() => setFilter("issues")}>
                只看有问题的
              </button>
            </div>
          )}

          <div className="toolbar">
            <label className="search-box">
              <Search size={15} />
              <input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="搜索模组"
                aria-label="搜索模组"
              />
            </label>
            <div className="filter-pills">
              {(
                [
                  ["all", "全部"],
                  ["enabled", "已启用"],
                  ["disabled", "已禁用"],
                  ["issues", "有问题"],
                ] as Array<[ModFilter, string]>
              ).map(([value, label]) => (
                <button
                  type="button"
                  key={value}
                  className={cx("filter-pill", filter === value && "is-active")}
                  onClick={() => setFilter(value)}
                >
                  {label}
                </button>
              ))}
            </div>
            <span className="result-count">{filtered.length} 个模组</span>
          </div>

          {mods.length === 0 ? (
            <section className="panel">
              <EmptyState
                art={<Package size={30} />}
                title="还没有安装模组"
                description="从 Modrinth 下载，或选择一个本地 .jar 文件加入当前版本。"
                action={
                  <div className="empty-actions">
                    <button
                      type="button"
                      className="btn btn-secondary"
                      onClick={() => openDownloads("mods")}
                    >
                      <DownloadCloud size={15} />
                      下载 Mod
                    </button>
                    <button type="button" className="btn btn-primary" onClick={() => void importMods()}>
                      <Plus size={15} />
                      安装模组
                    </button>
                  </div>
                }
              />
            </section>
          ) : filtered.length === 0 ? (
            <section className="panel">
              <EmptyState art={<Search size={28} />} title="没有匹配的模组" description="换一个搜索词或筛选条件。" />
            </section>
          ) : (
            <section className="panel data-panel">
              <div className="data-head mod-table">
                <span>模组</span>
                <span>加载器</span>
                <span>状态</span>
                <span />
              </div>
              {filtered.map((mod) => {
                const issue =
                  mod.missingDependencies.length > 0 || mod.duplicate || Boolean(mod.readError);
                const busy = busyFile === mod.fileName;
                return (
                  <div className={cx("data-row mod-table", !mod.enabled && "is-disabled")} key={mod.fileName}>
                    <div className="mod-primary">
                      <span
                        className="mod-tile"
                        style={{
                          background: `linear-gradient(135deg, ${modColor(mod.name)}, ${modColor(
                            mod.modId ?? mod.name,
                          )})`,
                        }}
                      >
                        {modInitial(mod.name)}
                      </span>
                      <div className="mod-copy">
                        <div className="mod-title">
                          <strong>{mod.name}</strong>
                          {mod.version && <span>v{mod.version}</span>}
                          {mod.duplicate && <span className="tag tag-danger">重复</span>}
                        </div>
                        <span className="mod-description">
                          {mod.description || mod.fileName}
                        </span>
                        <span className="mod-meta">
                          {mod.modId ?? "未知 ID"} · {formatBytes(mod.size)} ·{" "}
                          {formatRelative(new Date(mod.modified).toISOString())}
                        </span>
                        {issue && (
                          <span className="mod-warning">
                            <AlertTriangle size={12} />
                            {mod.readError
                              ? "无法读取模组信息"
                              : mod.missingDependencies.length > 0
                                ? `缺少依赖：${mod.missingDependencies.join("、")}`
                                : "存在重复的 mod ID"}
                          </span>
                        )}
                      </div>
                    </div>
                    <div>
                      <LoaderBadge loader={mod.loader === "unknown" ? "unknown" : mod.loader} compact />
                    </div>
                    <div className="mod-toggle">
                      <Toggle
                        checked={mod.enabled}
                        disabled={busy}
                        onChange={(next) => void toggle(mod, next)}
                        label={`${mod.enabled ? "禁用" : "启用"} ${mod.name}`}
                      />
                      <span>{mod.enabled ? "启用" : "禁用"}</span>
                    </div>
                    <div className="row-actions">
                      <button
                        type="button"
                        className="icon-button subtle danger-hover"
                        title="删除模组"
                        disabled={busy}
                        onClick={() => void remove(mod)}
                      >
                        <Trash2 size={15} />
                      </button>
                    </div>
                  </div>
                );
              })}
            </section>
          )}

          <p className="page-note">
            <ShieldCheck size={13} />
            依赖检查基于本地模组元数据；文件修改会在下次启动游戏时生效。
          </p>
        </>
      )}
    </div>
  );
}
