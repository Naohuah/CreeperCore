import { useEffect, useRef, useState } from "react";
import {
  AlertTriangle,
  ChevronRight,
  Download,
  Loader2,
  RefreshCw,
  Search,
} from "lucide-react";
import { api } from "../lib/bridge";
import { categoryLabel, errorMessage, formatRelative, versionTitle } from "../lib/format";
import { useAppStore } from "../store";
import type { ModrinthHit, VersionSummary } from "../lib/types";
import { EmptyState } from "./Ui";
import { ModDetailDialog } from "./ModDetailDialog";

const MODDED_LOADERS = new Set(["fabric", "quilt", "forge", "neoforge"]);

interface ModrinthBrowserProps {
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

export function ModrinthBrowser({
  versions,
  currentId,
  onNeedGameVersions,
}: ModrinthBrowserProps) {
  const toast = useAppStore((s) => s.toast);
  const modded = versions.filter((version) => MODDED_LOADERS.has(version.loader));
  const defaultId =
    modded.find((version) => version.id === currentId)?.id ?? modded[0]?.id ?? "";
  const [targetId, setTargetId] = useState(defaultId);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<ModrinthHit[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [translations, setTranslations] = useState<Record<string, string>>({});
  const [detailHit, setDetailHit] = useState<ModrinthHit | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const requestRef = useRef(0);
  const target = modded.find((version) => version.id === targetId);

  useEffect(() => {
    if (modded.some((version) => version.id === targetId)) return;
    setTargetId(modded.find((version) => version.id === currentId)?.id ?? modded[0]?.id ?? "");
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
        const page = await api.searchModrinth(targetId, query, 0, 20);
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
      const page = await api.searchModrinth(targetId, query, hits.length, 20);
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

  if (modded.length === 0) {
    return (
      <section className="panel">
        <EmptyState
          art={<AlertTriangle size={30} />}
          title="还没有模组加载器版本"
          description="Modrinth 的模组需要 Fabric、Quilt、Forge 或 NeoForge。先安装一个带加载器的游戏版本。"
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
    <div className="modrinth-browser">
      <div className="panel download-filter modrinth-toolbar">
        <div className="modrinth-target">
          <span className="field-label">版本</span>
          <div className="select-wrap">
            <select
              value={targetId}
              onChange={(event) => setTargetId(event.target.value)}
              aria-label="选择目标版本"
            >
              {modded.map((version) => (
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
            placeholder="搜索 Mod"
            aria-label="搜索 Modrinth 模组"
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
          <button type="button" className="text-button" onClick={() => setRefreshKey((value) => value + 1)}>
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
            正在搜索 Modrinth…
          </div>
        ) : hits.length === 0 && !loading ? (
          <EmptyState
            art={<Search size={28} />}
            title="没有找到模组"
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
              <div
                className="modrinth-row is-clickable"
                key={hit.projectId}
                role="button"
                tabIndex={0}
                title="查看版本"
                onClick={() => setDetailHit(hit)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    setDetailHit(hit);
                  }
                }}
              >
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
                      {hit.categories
                        .filter((category) =>
                          ["fabric", "quilt", "forge", "neoforge", "optimization", "shaders"].includes(
                            category,
                          ),
                        )
                        .slice(0, 3)
                        .map((category) => (
                          <span className="tag" key={category}>
                            {categoryLabel(category)}
                          </span>
                        ))}
                    </div>
                  </div>
                </div>
                <div className="modrinth-actions">
                  <button
                    type="button"
                    className="row-install"
                    onClick={(event) => {
                      event.stopPropagation();
                      setDetailHit(hit);
                    }}
                  >
                    <ChevronRight size={13} />
                    选择版本
                  </button>
                </div>
              </div>
            ))}
            {hits.length < total && (
              <button type="button" className="load-more" disabled={loadingMore} onClick={() => void loadMore()}>
                {loadingMore ? <Loader2 size={13} className="spin" /> : null}
                {loadingMore ? "正在加载…" : "显示更多"}
              </button>
            )}
          </>
        )}
      </section>
      <ModDetailDialog
        open={detailHit !== null}
        onClose={() => setDetailHit(null)}
        targetVersionId={targetId}
        targetLabel={
          target ? versionTitle(target.id, target.inheritsFrom, target.loader) : targetId
        }
        hit={detailHit}
        translatedDescription={
          detailHit ? translations[detailHit.projectId] : undefined
        }
      />
    </div>
  );
}
