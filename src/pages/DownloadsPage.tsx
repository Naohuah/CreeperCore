import { useCallback, useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  ArrowUpRight,
  ChevronRight,
  Download,
  Loader2,
  RefreshCw,
  Search,
  X,
} from "lucide-react";
import { useAppStore } from "../store";
import { api } from "../lib/bridge";
import { cx, errorMessage, formatDate, typeLabel } from "../lib/format";
import { StatusPill } from "../components/Ui";
import { ContentBrowser } from "../components/ContentBrowser";
import { ModrinthBrowser } from "../components/ModrinthBrowser";
import { ModpackBrowser } from "../components/ModpackBrowser";
import type { RemoteVersion, VersionManifest } from "../lib/types";

type Channel = "release" | "snapshot" | "all";
type LoaderMode = "vanilla" | "fabric" | "forge" | "neoforge";

export function DownloadsPage() {
  const boot = useAppStore((s) => s.boot);
  const install = useAppStore((s) => s.install);
  const contentTask = useAppStore((s) => s.contentTask);
  const cancelInstall = useAppStore((s) => s.cancelInstall);
  const startInstall = useAppStore((s) => s.startInstall);
  const navigate = useAppStore((s) => s.navigate);
  const toast = useAppStore((s) => s.toast);
  const selectedInstalled = useAppStore((s) => s.boot?.config.selectedVersion);
  const communitySource = useAppStore(
    (s) => s.boot?.config.communitySource ?? "official",
  );
  const [manifest, setManifest] = useState<VersionManifest | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [channel, setChannel] = useState<Channel>("release");
  const [loaderMode, setLoaderMode] = useState<LoaderMode>("vanilla");
  const [visible, setVisible] = useState(24);
  const section = useAppStore((s) => s.downloadSection);
  const setSection = useAppStore((s) => s.setDownloadSection);

  const loadManifest = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setManifest(await api.fetchRemoteVersions());
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadManifest();
  }, [loadManifest]);

  const installedSet = useMemo(() => {
    const map = new Map<string, string>();
    for (const version of boot?.versions ?? []) {
      if (version.loader !== "fabric") {
        map.set(version.id, version.id);
      }
    }
    return map;
  }, [boot?.versions]);

  const filtered = useMemo(() => {
    if (!manifest) return [];
    return manifest.versions.filter((version) => {
      if (query.trim() && !version.id.toLowerCase().includes(query.trim().toLowerCase())) {
        return false;
      }
      if (channel === "release") return version.type === "release";
      if (channel === "snapshot") return version.type === "snapshot";
      return true;
    });
  }, [manifest, query, channel]);

  const busy = install?.running ?? false;

  const matchedInstalled = (remote: RemoteVersion): string | undefined =>
    loaderMode === "vanilla"
      ? installedSet.get(remote.id)
      : boot?.versions.find(
          (version) =>
            version.loader === loaderMode &&
            (version.inheritsFrom === remote.id || version.id.endsWith(`-${remote.id}`)),
        )?.id;

  const installRemote = async (remote: RemoteVersion) => {
    if (busy) {
      toast("warning", "已有任务正在进行", "请等待当前安装完成");
      return;
    }
    await startInstall(remote.id, loaderMode);
  };

  return (
    <div className="page downloads-page">
      <div className="download-topbar">
        <div className="segmented download-tabs">
          <button
            type="button"
            className={cx(section === "game" && "is-active")}
            onClick={() => setSection("game")}
          >
            游戏
          </button>
          <button
            type="button"
            className={cx(section === "mods" && "is-active")}
            onClick={() => setSection("mods")}
          >
            Mod
          </button>
          <button
            type="button"
            className={cx(section === "packs" && "is-active")}
            onClick={() => setSection("packs")}
          >
            整合包
          </button>
          <button
            type="button"
            className={cx(section === "resourcepack" && "is-active")}
            onClick={() => setSection("resourcepack")}
          >
            资源包
          </button>
          <button
            type="button"
            className={cx(section === "shader" && "is-active")}
            onClick={() => setSection("shader")}
          >
            光影
          </button>
          <button
            type="button"
            className={cx(section === "datapack" && "is-active")}
            onClick={() => setSection("datapack")}
          >
            数据包
          </button>
        </div>
        <div className="download-source">
          <span className="source-chip">
            {section === "game"
              ? boot?.config.downloadSource === "bmclapi"
                ? "BMCLAPI"
                : "MOJANG"
              : communitySource === "mcim"
                ? "MCIM"
                : "MODRINTH"}
          </span>
          {section === "game" && (
            <button
              type="button"
              className="btn btn-ghost btn-sm icon-only"
              title="刷新版本清单"
              aria-label="刷新版本清单"
              onClick={() => void loadManifest()}
              disabled={loading}
            >
              <RefreshCw size={14} className={loading ? "spin" : undefined} />
            </button>
          )}
        </div>
      </div>

      {(install?.running || contentTask) && (
        <div className="download-task">
          <span className="download-task-icon">
            <Loader2 size={17} className="spin" />
          </span>
          <div className="download-task-copy">
            <strong>
              {contentTask
                ? contentTask.title
                : install?.loader === "fabric"
                  ? "正在安装 Fabric"
                  : install?.loader === "forge"
                    ? "正在安装 Forge"
                    : install?.loader === "neoforge"
                      ? "正在安装 NeoForge"
                      : "正在安装游戏版本"}
            </strong>
            <span>
              {contentTask
                ? contentTask.detail
                : install?.progress
                  ? `${install.progress.label} · ${install.progress.percent.toFixed(0)}%`
                  : "正在准备下载"}
            </span>
          </div>
          <button
            type="button"
            className="btn btn-secondary btn-sm"
            onClick={() =>
              void (contentTask
                ? api.cancelModrinthInstall()
                : cancelInstall())
            }
          >
            <X size={13} />
            取消下载
          </button>
        </div>
      )}

      {section === "mods" ? (
        <ModrinthBrowser
          versions={boot?.versions ?? []}
          currentId={selectedInstalled}
          onNeedGameVersions={() => setSection("game")}
        />
      ) : section === "packs" ? (
        <ModpackBrowser
          versions={boot?.versions ?? []}
          currentId={selectedInstalled}
          onNeedGameVersions={() => setSection("game")}
        />
      ) : section === "resourcepack" || section === "shader" || section === "datapack" ? (
        <ContentBrowser
          kind={section}
          versions={boot?.versions ?? []}
          currentId={selectedInstalled}
          onNeedGameVersions={() => setSection("game")}
        />
      ) : (
        <>
      <div className="panel download-filter">
        <div className="segmented segmented-sm">
          <button
            type="button"
            className={cx(loaderMode === "vanilla" && "is-active")}
            onClick={() => setLoaderMode("vanilla")}
          >
            原版
          </button>
          <button
            type="button"
            className={cx(loaderMode === "fabric" && "is-active")}
            onClick={() => setLoaderMode("fabric")}
          >
            Fabric
          </button>
          <button
            type="button"
            className={cx(loaderMode === "forge" && "is-active")}
            onClick={() => setLoaderMode("forge")}
          >
            Forge
          </button>
          <button
            type="button"
            className={cx(loaderMode === "neoforge" && "is-active")}
            onClick={() => setLoaderMode("neoforge")}
          >
            NeoForge
          </button>
        </div>
        <label className="search-box">
          <Search size={15} />
          <input
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setVisible(24);
            }}
            placeholder="搜索版本"
            aria-label="搜索官方版本"
          />
        </label>
        <div className="filter-pills">
          {(
            [
              ["release", "正式版"],
              ["snapshot", "快照"],
              ["all", "全部"],
            ] as Array<[Channel, string]>
          ).map(([value, label]) => (
            <button
              type="button"
              key={value}
              className={cx("filter-pill", channel === value && "is-active")}
              onClick={() => {
                setChannel(value);
                setVisible(24);
              }}
            >
              {label}
            </button>
          ))}
        </div>
      </div>

      {error && (
        <div className="inline-alert warn">
          <AlertTriangle size={16} />
          <div>
            <strong>无法获取版本清单</strong>
            <span>{error}。已安装的版本不受影响。</span>
          </div>
          <button type="button" className="text-button" onClick={() => void loadManifest()}>
            重试
          </button>
        </div>
      )}

      {loading ? (
        <div className="panel loading-panel">
          <Loader2 size={20} className="spin" />
          正在获取版本清单…
        </div>
      ) : (
        <section className="panel data-panel">
          {filtered.length === 0 ? (
            <div className="table-empty">没有找到匹配的版本。</div>
          ) : (
            filtered.slice(0, visible).map((remote) => {
              const installedId = matchedInstalled(remote);
              const current = installedId && installedId === selectedInstalled;
              return (
                <div className="data-row remote-table" key={remote.id}>
                  <div className="remote-version-name">
                    <strong>{remote.id}</strong>
                    {remote.type !== "release" && (
                      <span className={cx("tag", `tag-${remote.type}`)}>{typeLabel(remote.type)}</span>
                    )}
                  </div>
                  <span className="cell-muted">{formatDate(remote.releaseTime)}</span>
                  <span>
                    {installedId ? (
                      <StatusPill tone={current ? "ok" : "info"}>
                        {current ? "当前版本" : "已安装"}
                      </StatusPill>
                    ) : (
                      <span className="cell-muted">未安装</span>
                    )}
                  </span>
                  <div className="row-actions">
                    {installedId ? (
                      <button
                        type="button"
                        className="text-button"
                        onClick={() => {
                          if (current) navigate("launch");
                          else void useAppStore.getState().selectVersion(installedId);
                        }}
                      >
                        {current ? "去启动" : "设为当前"}
                        <ArrowUpRight size={12} />
                      </button>
                    ) : (
                      <button
                        type="button"
                        className="row-install"
                        disabled={busy}
                        onClick={() => void installRemote(remote)}
                      >
                        <Download size={12} />
                        安装
                      </button>
                    )}
                  </div>
                </div>
              );
            })
          )}
          {visible < filtered.length && (
            <button
              type="button"
              className="load-more"
              onClick={() => setVisible((value) => value + 24)}
            >
              显示更多
              <ChevronRight size={13} />
            </button>
          )}
        </section>
      )}

        </>
      )}
    </div>
  );
}
