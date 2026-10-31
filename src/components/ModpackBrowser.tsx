import { useEffect, useRef, useState } from "react";
import { AlertTriangle, Download, Loader2, RefreshCw, Search, X } from "lucide-react";
import { api } from "../lib/bridge";
import { errorMessage, formatRelative, versionTitle } from "../lib/format";
import { useAppStore } from "../store";
import type { ModrinthHit, VersionSummary } from "../lib/types";
import { EmptyState } from "./Ui";

interface ModpackBrowserProps {
  versions: VersionSummary[];
  currentId?: string | null;
  onNeedGameVersions: () => void;
}

function compactNumber(value: number): string {
  return new Intl.NumberFormat("zh-CN", { notation: "compact", maximumFractionDigits: 1 }).format(
    value,
  );
}

function initial(title: string): string {
  return (title.trim()[0] ?? "?").toUpperCase();
}

export function ModpackBrowser({ versions, currentId, onNeedGameVersions }: ModpackBrowserProps) {
  const refresh = useAppStore((s) => s.refresh);
  const navigate = useAppStore((s) => s.navigate);
  const toast = useAppStore((s) => s.toast);
  const fabricVersions = versions.filter((version) => version.loader === "fabric");
  const defaultId =
    fabricVersions.find((version) => version.id === currentId)?.id ?? fabricVersions[0]?.id ?? "";
  const [targetId, setTargetId] = useState(defaultId);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<ModrinthHit[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [translations, setTranslations] = useState<Record<string, string>>({});
  const requestRef = useRef(0);
  const target = fabricVersions.find((version) => version.id === targetId);

  useEffect(() => {
    if (fabricVersions.some((version) => version.id === targetId)) return;
    setTargetId(
      fabricVersions.find((version) => version.id === currentId)?.id ?? fabricVersions[0]?.id ?? "",
    );
  }, [versions, currentId, targetId]);

  useEffect(() => {
    if (!targetId) {
      setHits([]);
      setTotal(0);
      return;
    }
    const request = ++requestRef.current;
    const timer = window.setTimeout(async () => {
      setLoading(true);
      setError(null);
      try {
        const page = await api.searchModrinthModpacks(targetId, query, 0, 20);
        if (requestRef.current !== request) return;
        setHits(page.hits);
        setTotal(page.totalHits);
        const ids = page.hits.map((hit) => hit.projectId);
        if (ids.length > 0) {
          void api
            .translateModrinthSummaries(ids)
            .then((map) => {
              if (requestRef.current === request) {
                setTranslations((current) => ({ ...current, ...map }));
              }
            })
            .catch(() => undefined);
        }
      } catch (err) {
        if (requestRef.current !== request) return;
        setError(errorMessage(err));
        setHits([]);
        setTotal(0);
      } finally {
        if (requestRef.current === request) setLoading(false);
      }
    }, 300);
    return () => window.clearTimeout(timer);
  }, [targetId, query, refreshKey]);

  const loadMore = async () => {
    if (!targetId || loadingMore || hits.length >= total) return;
    setLoadingMore(true);
    try {
      const page = await api.searchModrinthModpacks(targetId, query, hits.length, 20);
      setHits((current) => [...current, ...page.hits]);
      setTotal(page.totalHits);
      const ids = page.hits.map((hit) => hit.projectId);
      if (ids.length > 0) {
        void api
          .translateModrinthSummaries(ids)
          .then((map) => setTranslations((current) => ({ ...current, ...map })))
          .catch(() => undefined);
      }
    } catch (err) {
      toast("error", "加载更多失败", errorMessage(err));
    } finally {
      setLoadingMore(false);
    }
  };

  const setContentTask = useAppStore((s) => s.setContentTask);

  const install = async (hit: ModrinthHit) => {
    if (!targetId || installing) return;
    setInstalling(hit.projectId);
    setContentTask({
      title: `安装整合包 ${hit.title}`,
      detail: "正在下载并解压整合包",
    });
    try {
      const report = await api.installModrinthModpack(targetId, hit.projectId);
      if (report.warnings.length > 0) {
        toast("warning", "整合包安装提示", report.warnings.slice(0, 2).join("；"));
      }
      if (report.filesInstalled === 0) {
        toast("warning", "整合包没有安装文件", "请检查网络或 Modrinth 状态");
        return;
      }
      toast(
        "success",
        "整合包已安装",
        `${report.packName} · ${report.filesInstalled} 个文件 · ${report.overrides} 个覆盖文件`,
      );
      await refresh(true);
      navigate("launch");
    } catch (err) {
      const message = errorMessage(err);
      if (message.includes("取消")) {
        toast("info", "已取消下载", hit.title);
      } else {
        toast("error", "整合包安装失败", message);
      }
    } finally {
      setInstalling(null);
      setContentTask(null);
    }
  };

  if (fabricVersions.length === 0) {
    return (
      <section className="panel">
        <EmptyState
          art={<AlertTriangle size={30} />}
          title="还没有 Fabric 版本"
          description="整合包需要一个已经安装的 Fabric 基础版本。先安装 Fabric，再回来下载整合包。"
          action={
            <button type="button" className="btn btn-primary" onClick={onNeedGameVersions}>
              去安装 Fabric 版本
            </button>
          }
        />
      </section>
    );
  }

  return (
    <div className="modrinth-browser modpack-browser">
      <div className="panel download-filter modrinth-toolbar">
        <div className="modrinth-target">
          <span className="field-label">版本</span>
          <div className="select-wrap">
            <select
              value={targetId}
              onChange={(event) => setTargetId(event.target.value)}
              aria-label="选择整合包基础版本"
            >
              {fabricVersions.map((version) => (
                <option key={version.id} value={version.id}>
                  {versionTitle(version.id, version.inheritsFrom, version.loader)}
                </option>
              ))}
            </select>
          </div>
        </div>
        <label className="search-box modrinth-search">
          <Search size={15} />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索整合包"
            aria-label="搜索 Modrinth 整合包"
          />
        </label>
        <button
          type="button"
          className="btn btn-ghost"
          disabled={loading}
          onClick={() => setRefreshKey((value) => value + 1)}
          title="刷新搜索结果"
        >
          <RefreshCw size={15} className={loading ? "spin" : undefined} />
        </button>
      </div>

      {error ? (
        <div className="inline-alert warn">
          <AlertTriangle size={16} />
          <div>
            <strong>无法搜索 Modrinth</strong>
            <span>{error}</span>
          </div>
          <button
            type="button"
            className="text-button"
            onClick={() => setRefreshKey((value) => value + 1)}
          >
            重试
          </button>
        </div>
      ) : null}

      <section className="panel modrinth-list">
        {error ? (
          <div className="table-empty">Modrinth 搜索失败，请检查网络后重试。</div>
        ) : loading && hits.length === 0 ? (
          <div className="loading-panel">
            <Loader2 size={19} className="spin" />
            正在搜索整合包…
          </div>
        ) : hits.length === 0 && !loading ? (
          <EmptyState
            art={<Search size={28} />}
            title="没有找到整合包"
            description={
              target
                ? `当前筛选：Minecraft ${versionTitle(target.id, target.inheritsFrom, target.loader)} · Fabric`
                : "换一个搜索词，或检查基础版本。"
            }
            action={
              query ? (
                <button type="button" className="btn btn-ghost" onClick={() => setQuery("")}>
                  清除搜索
                </button>
              ) : undefined
            }
          />
        ) : (
          <>
            {hits.map((hit) => (
              <div className="modrinth-row" key={hit.projectId}>
                <span className="modrinth-icon">
                  <span className="modrinth-icon-fallback">{initial(hit.title)}</span>
                  {hit.iconUrl && (
                    <img
                      src={hit.iconUrl}
                      alt=""
                      loading="lazy"
                      referrerPolicy="no-referrer"
                      onError={(event) => {
                        event.currentTarget.style.display = "none";
                      }}
                    />
                  )}
                </span>
                <div className="modrinth-copy">
                  <div className="modrinth-title">
                    <strong>{hit.title}</strong>
                    <span>by {hit.author || "未知作者"}</span>
                  </div>
                  <p title={hit.description}>{translations[hit.projectId] ?? hit.description}</p>
                  <div className="modrinth-row-meta">
                    <span>
                      <Download size={11} />
                      {compactNumber(hit.downloads)}
                    </span>
                    <span>{hit.dateModified ? formatRelative(hit.dateModified) : "时间未知"}</span>
                    <div className="modrinth-tags">
                      <span className="tag">Fabric</span>
                    </div>
                  </div>
                </div>
                <div className="modrinth-actions">
                  <button
                    type="button"
                    className="row-install"
                    disabled={installing !== null && installing !== hit.projectId}
                    onClick={() => {
                      if (installing === hit.projectId) {
                        void api.cancelModrinthInstall().catch(() => {});
                      } else {
                        void install(hit);
                      }
                    }}
                  >
                    {installing === hit.projectId ? (
                      <X size={13} />
                    ) : (
                      <Download size={13} />
                    )}
                    {installing === hit.projectId ? "取消" : "安装"}
                  </button>
                </div>
              </div>
            ))}
            {hits.length < total && (
              <button
                type="button"
                className="load-more"
                disabled={loadingMore}
                onClick={() => void loadMore()}
              >
                {loadingMore ? <Loader2 size={13} className="spin" /> : null}
                {loadingMore ? "正在加载…" : "显示更多"}
              </button>
            )}
          </>
        )}
      </section>
    </div>
  );
}
