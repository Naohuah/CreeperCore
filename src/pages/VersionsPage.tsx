import { useMemo, useState } from "react";
import {
  AlertTriangle,
  Check,
  Download,
  FolderOpen,
  Play,
  Plus,
  RefreshCw,
  Search,
  Settings2,
  Trash2,
} from "lucide-react";
import { useAppStore } from "../store";
import { api } from "../lib/bridge";
import {
  cx,
  errorMessage,
  loaderLabel,
  shortPath,
  versionTitle,
} from "../lib/format";
import { CreeperMark } from "../components/CreeperMark";
import { EmptyState, LoaderBadge, PageHeader, StatusPill } from "../components/Ui";
import { InstanceDialog } from "../components/InstanceDialog";

type Filter = "all" | "vanilla" | "modded" | "snapshot";

export function VersionsPage() {
  const boot = useAppStore((s) => s.boot);
  const game = useAppStore((s) => s.game);
  const navigate = useAppStore((s) => s.navigate);
  const launch = useAppStore((s) => s.launch);
  const selectVersion = useAppStore((s) => s.selectVersion);
  const refresh = useAppStore((s) => s.refresh);
  const askConfirm = useAppStore((s) => s.askConfirm);
  const toast = useAppStore((s) => s.toast);
  const showOverlay = useAppStore((s) => s.showOverlay);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [manageId, setManageId] = useState<string | null>(null);

  const versions = boot?.versions ?? [];
  const selectedId = boot?.config.selectedVersion;

  const filtered = useMemo(() => {
    return versions.filter((version) => {
      if (query.trim() && !version.id.toLowerCase().includes(query.trim().toLowerCase())) {
        return false;
      }
      if (filter === "vanilla") return version.loader === "vanilla";
      if (filter === "modded")
        return version.loader !== "vanilla" && version.loader !== "unknown";
      if (filter === "snapshot") return version.versionType === "snapshot";
      return true;
    });
  }, [versions, query, filter]);

  const openFolder = async (versionId: string) => {
    try {
      await api.openPath("version", versionId);
    } catch (error) {
      toast("error", "无法打开版本目录", errorMessage(error));
    }
  };

  const removeVersion = async (versionId: string) => {
    const accepted = await askConfirm({
      title: `删除版本 ${versionId}？`,
      description:
        "版本目录会被永久删除，无法恢复。共享的 assets 与 libraries 不受影响。",
      confirmLabel: "永久删除",
      danger: true,
    });
    if (!accepted) return;
    try {
      await api.deleteVersion(versionId);
      await refresh(true);
      toast("success", "版本已删除");
    } catch (error) {
      toast("error", "无法删除版本", errorMessage(error));
    }
  };

  if (!boot) return null;

  return (
    <div className="page versions-page">
      <PageHeader
        title="版本管理"
        description="管理已安装的版本、选择当前版本，或安装新的版本。"
        actions={
          <div className="page-action-row">
            <button type="button" className="btn btn-ghost" onClick={() => void refresh(true)}>
              <RefreshCw size={15} />
              刷新
            </button>
            <button type="button" className="btn btn-primary" onClick={() => navigate("downloads")}>
              <Plus size={16} />
              安装版本
            </button>
          </div>
        }
      />

      <div className="path-bar">
        <FolderOpen size={15} />
        <span>游戏目录</span>
        <code>{shortPath(boot.config.gameDir, 72)}</code>
        <button
          type="button"
          className="text-button"
          onClick={() =>
            void api.openPath("root", null).catch((error) =>
              toast("error", "无法打开目录", errorMessage(error)),
            )
          }
        >
          打开
        </button>
        <button type="button" className="text-button" onClick={() => navigate("settings")}>
          更改
        </button>
      </div>

      <div className="toolbar">
        <label className="search-box">
          <Search size={15} />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索版本"
            aria-label="搜索版本"
          />
        </label>
        <div className="filter-pills">
          {(
            [
              ["all", "全部"],
              ["vanilla", "原版"],
              ["modded", "模组"],
              ["snapshot", "快照"],
            ] as Array<[Filter, string]>
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
        <span className="result-count">{filtered.length} 个版本</span>
      </div>

      {versions.length === 0 ? (
        <section className="panel">
          <EmptyState
            art={<CreeperMark size={56} />}
            title="还没有安装任何版本"
            description="下载一个官方版本，或者将游戏目录指向已有的 .minecraft。"
            action={
              <button type="button" className="btn btn-primary" onClick={() => navigate("downloads")}>
                <Download size={15} />
                下载版本
              </button>
            }
          />
        </section>
      ) : filtered.length === 0 ? (
        <section className="panel">
          <EmptyState
            art={<Search size={28} />}
            title="没有匹配的版本"
            description="换一个搜索词，或者切换筛选条件。"
            action={
              <button
                type="button"
                className="btn btn-ghost"
                onClick={() => {
                  setQuery("");
                  setFilter("all");
                }}
              >
                清除筛选
              </button>
            }
          />
        </section>
      ) : (
        <section className="panel data-panel">
          <div className="data-head version-table">
            <span>版本</span>
            <span>加载器</span>
            <span>Java</span>
            <span>状态</span>
            <span>操作</span>
          </div>
          {filtered.map((version) => {
            const active = version.id === selectedId;
            const runningThis = game.running && game.versionId === version.id;
            return (
              <div className={cx("data-row version-table", active && "is-current")} key={version.id}>
                <div className="version-primary">
                  <span className={cx("version-avatar", `is-${version.loader}`)}>
                    {loaderLabel(version.loader).slice(0, 1)}
                  </span>
                  <div>
                    <strong>{versionTitle(version.id, version.inheritsFrom, version.loader)}</strong>
                    <span>{version.id}</span>
                  </div>
                </div>
                <div>
                  <LoaderBadge loader={version.loader} version={version.loaderVersion} compact />
                </div>
                <span className="cell-muted">Java {version.javaMajor ?? "自动"}</span>
                <div className="version-state">
                  {runningThis ? (
                    <StatusPill tone="ok">运行中</StatusPill>
                  ) : active ? (
                    <StatusPill tone="ok">当前版本</StatusPill>
                  ) : !version.hasClient ? (
                    <span className="text-warning">
                      <AlertTriangle size={13} /> 文件不全
                    </span>
                  ) : version.versionType === "snapshot" ? (
                    <span className="cell-muted">快照 · 已安装</span>
                  ) : (
                    <span className="cell-muted">已安装</span>
                  )}
                </div>
                <div className="row-actions">
                  <button
                    type="button"
                    className={cx("btn btn-sm", active ? "btn-primary" : "btn-secondary")}
                    onClick={() =>
                      runningThis ? showOverlay() : void launch(version.id)
                    }
                  >
                    <Play size={12} fill="currentColor" />
                    {runningThis ? "回到游戏" : "启动"}
                  </button>
                  {!active && (
                    <button
                      type="button"
                      className="btn btn-sm btn-ghost"
                      onClick={() => void selectVersion(version.id)}
                    >
                      设为当前
                    </button>
                  )}
                  {active && (
                    <span className="current-mark">
                      <Check size={13} /> 当前
                    </span>
                  )}
                  <button
                    type="button"
                    className="icon-button subtle"
                    title="实例管理"
                    onClick={() => setManageId(version.id)}
                  >
                    <Settings2 size={15} />
                  </button>
                  <button
                    type="button"
                    className="icon-button subtle"
                    title="打开版本目录"
                    onClick={() => void openFolder(version.id)}
                  >
                    <FolderOpen size={15} />
                  </button>
                  <button
                    type="button"
                    className="icon-button subtle danger-hover"
                    title="移除版本"
                    onClick={() => void removeVersion(version.id)}
                  >
                    <Trash2 size={15} />
                  </button>
                </div>
              </div>
            );
          })}
        </section>
      )}

      <InstanceDialog
        open={manageId !== null}
        versionId={manageId ?? ""}
        onClose={() => setManageId(null)}
      />
    </div>
  );
}
