import { useEffect, useRef, useState } from "react";
import {
  AlertTriangle,
  Check,
  CheckCircle2,
  Download,
  FolderOpen,
  Loader2,
  RefreshCw,
  Search,
  X,
} from "lucide-react";
import { api } from "../lib/bridge";
import { categoryLabel, errorMessage, formatRelative, versionTitle } from "../lib/format";
import { useAppStore } from "../store";
import type { ModrinthHit, VersionSummary } from "../lib/types";
import { EmptyState } from "./Ui";

export type ContentKind = "resourcepack" | "shader" | "datapack";

const KIND_META: Record<
  ContentKind,
  { label: string; placeholder: string; folder: string; empty: string }
> = {
  resourcepack: {
    label: "资源包",
    placeholder: "搜索资源包",
    folder: "resourcepacks",
    empty: "没有找到资源包",
  },
  shader: {
    label: "光影",
    placeholder: "搜索光影",
    folder: "shaderpacks",
    empty: "没有找到光影",
  },
  datapack: {
    label: "数据包",
    placeholder: "搜索数据包",
    folder: "datapacks",
    empty: "没有找到数据包",
  },
};

function compactNumber(value: number): string {
  return new Intl.NumberFormat("zh-CN", { notation: "compact", maximumFractionDigits: 1 }).format(
    value,
  );
}

function initial(title: string): string {
  return (title.trim()[0] ?? "?").toUpperCase();
}

interface ContentBrowserProps {
  kind: ContentKind;
  versions: VersionSummary[];
  currentId?: string | null;
  onNeedGameVersions: () => void;
}

export function ContentBrowser({
  kind,
  versions,
  currentId,
  onNeedGameVersions,
}: ContentBrowserProps) {
  const toast = useAppStore((s) => s.toast);
  const meta = KIND_META[kind];
  const currentTarget = versions.find((version) => version.id === currentId)?.id;
  const [targetId, setTargetId] = useState(currentTarget ?? versions[0]?.id ?? "");
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<ModrinthHit[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);
  const [installed, setInstalled] = useState<Set<string>>(new Set());
  const [lastResult, setLastResult] = useState<{
    title: string;
    files: string[];
    alreadyExisted: boolean;
  } | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [translations, setTranslations] = useState<Record<string, string>>({});
  const requestRef = useRef(0);
  const target = versions.find((version) => version.id === targetId);

  useEffect(() => {
    if (versions.some((version) => version.id === targetId)) return;
    setTargetId(
      versions.find((version) => version.id === currentId)?.id ?? versions[0]?.id ?? "",
    );
  }, [versions, currentId, targetId]);

  useEffect(() => {
    setInstalled(new Set());
    setLastResult(null);
  }, [targetId]);

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
        const page = await api.searchModrinthContent(targetId, kind, query, 0, 20);
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
  }, [targetId, query, refreshKey, kind]);

  const loadMore = async () => {
    if (!targetId || loadingMore || hits.length >= total) return;
    setLoadingMore(true);
    try {
      const page = await api.searchModrinthContent(targetId, kind, query, hits.length, 20);
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
      title: `安装 ${hit.title}`,
      detail: `正在下载到 ${meta.label}`,
    });
    try {
      const report = await api.installModrinthContent(targetId, hit.projectId, kind);
      const files = report.installed.length > 0 ? report.installed : report.skipped;
      if (files.length > 0) {
        setInstalled((current) => new Set(current).add(hit.projectId));
        setLastResult({
          title: hit.title,
          files,
          alreadyExisted: report.installed.length === 0,
        });
        toast(
          "success",
          report.installed.length > 0 ? `${meta.label}已安装` : "文件已经存在",
          `${files[0]} → ${meta.folder}`,
        );
      } else {
        toast("warning", "没有安装文件", hit.title);
      }
      if (report.warnings.length > 0) {
        toast("warning", "安装提示", report.warnings.slice(0, 2).join("；"));
      }
    } catch (err) {
      const message = errorMessage(err);
      if (message.includes("取消")) {
        toast("info", "已取消下载", hit.title);
      } else {
        toast("error", "安装失败", message);
      }
    } finally {
      setInstalling(null);
      setContentTask(null);
    }
  };

  const openFolder = async () => {
    try {
      await api.openPath(meta.folder, targetId);
    } catch (err) {
      toast("error", "无法打开目录", errorMessage(err));
    }
  };

  if (versions.length === 0) {
    return (
      <section className="panel">
        <EmptyState
          art={<AlertTriangle size={30} />}
          title="还没有游戏版本"
          description="资源包、光影和数据包都需要一个已安装的游戏版本作为安装目标。"
          action={
            <button type="button" className="btn btn-primary" onClick={onNeedGameVersions}>
              去安装游戏版本
            </button>
          }
        />
      </section>
    );
  }

  return (
    <div className="modrinth-browser content-browser">
      <div className="panel download-filter modrinth-toolbar">
        <div className="modrinth-target">
          <span className="field-label">版本</span>
          <div className="select-wrap">
            <select
              value={targetId}
              onChange={(event) => setTargetId(event.target.value)}
              aria-label={`选择${meta.label}目标版本`}
            >
              {versions.map((version) => (
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
            placeholder={meta.placeholder}
            aria-label={meta.placeholder}
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

      {lastResult && (
        <div className="content-result">
          <CheckCircle2 size={17} />
          <div className="content-result-copy">
            <strong>
              {lastResult.title}{" "}
              {lastResult.alreadyExisted ? "已经存在" : `已安装到 ${meta.label}`}
            </strong>
            <span>
              {lastResult.files.slice(0, 2).join("、")}
              {lastResult.files.length > 2 ? ` 等 ${lastResult.files.length} 个文件` : ""}
              {" → "}
              {meta.folder}
            </span>
          </div>
          <button type="button" className="text-button" onClick={() => void openFolder()}>
            <FolderOpen size={13} />
            打开文件夹
          </button>
          <button
            type="button"
            className="icon-button subtle"
            onClick={() => setLastResult(null)}
            aria-label="关闭安装提示"
          >
            <X size={14} />
          </button>
        </div>
      )}

      <section className="panel modrinth-list">
        {error ? (
          <div className="table-empty">Modrinth 搜索失败，请检查网络后重试。</div>
        ) : loading && hits.length === 0 ? (
          <div className="loading-panel">
            <Loader2 size={19} className="spin" />
            正在搜索{meta.label}…
          </div>
        ) : hits.length === 0 && !loading ? (
          <EmptyState
            art={<Search size={28} />}
            title={meta.empty}
            description={
              target
                ? `当前筛选：${versionTitle(target.id, target.inheritsFrom, target.loader)}`
                : "换一个搜索词，或检查目标版本。"
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
                      {hit.categories.slice(0, 2).map((category) => (
                        <span className="tag" key={category}>
                          {categoryLabel(category)}
                        </span>
                      ))}
                    </div>
                  </div>
                </div>
                <div className="modrinth-actions">
                  {installed.has(hit.projectId) ? (
                    <span className="installed-chip">
                      <Check size={12} />
                      已安装
                    </span>
                  ) : (
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
                  )}
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
      {kind === "datapack" && (
        <p className="content-hint">
          数据包会安装到游戏目录的 datapacks；如果只对单个世界生效，请把文件移到该存档的 datapacks 目录。
        </p>
      )}
    </div>
  );
}
