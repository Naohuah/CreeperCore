import { useEffect, useState } from "react";
import {
  AlertTriangle,
  BookOpen,
  Copy,
  Download,
  ExternalLink,
  Loader2,
  Package,
  X,
} from "lucide-react";
import { api } from "../lib/bridge";
import { cx, errorMessage, formatBytes, formatRelative } from "../lib/format";
import { useAppStore } from "../store";
import type {
  ModrinthHit,
  ModrinthProjectSummary,
  ModrinthVersionInfo,
} from "../lib/types";
import { Modal } from "./Modal";

interface ModDetailDialogProps {
  open: boolean;
  onClose: () => void;
  targetVersionId: string;
  targetLabel: string;
  hit: ModrinthHit | null;
  translatedDescription?: string;
}

const TYPE_LABEL: Record<string, string> = {
  release: "正式版",
  beta: "测试版",
  alpha: "预览版",
};

export function ModDetailDialog({
  open,
  onClose,
  targetVersionId,
  targetLabel,
  hit,
  translatedDescription,
}: ModDetailDialogProps) {
  const toast = useAppStore((s) => s.toast);
  const setContentTask = useAppStore((s) => s.setContentTask);
  const [versions, setVersions] = useState<ModrinthVersionInfo[]>([]);
  const [selectedVersionId, setSelectedVersionId] = useState<string | null>(null);
  const [prerequisites, setPrerequisites] = useState<
    Record<string, ModrinthProjectSummary>
  >({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [installing, setInstalling] = useState<string | null>(null);

  useEffect(() => {
    if (!open || !hit) return;
    let active = true;
    setLoading(true);
    setError(null);
    setVersions([]);
    setSelectedVersionId(null);
    setPrerequisites({});
    void api
      .listModrinthVersions(targetVersionId, hit.projectId)
      .then((result) => {
        if (!active) return;
        setVersions(result);
        setSelectedVersionId(result[0]?.id ?? null);
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
  }, [open, hit, targetVersionId]);

  const selectedVersion =
    versions.find((version) => version.id === selectedVersionId) ??
    versions[0] ??
    null;
  const dependencyIds = selectedVersion?.dependencyProjectIds ?? [];
  const dependencyKey = dependencyIds.join(",");

  useEffect(() => {
    if (!open || !dependencyKey) return;
    let active = true;
    const missing = dependencyIds.filter((id) => !prerequisites[id]);
    if (missing.length === 0) return;
    void api
      .listModrinthProjects(missing)
      .then((projects) => {
        if (!active) return;
        setPrerequisites((current) => {
          const next = { ...current };
          for (const project of projects) next[project.projectId] = project;
          return next;
        });
      })
      .catch(() => {
        // The project ids still provide a useful fallback in the panel.
      });
    return () => {
      active = false;
    };
  }, [open, dependencyKey]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!hit) return null;

  const openModrinth = () => {
    const slug = hit.slug || hit.projectId;
    void api
      .openExternal(`https://modrinth.com/mod/${encodeURIComponent(slug)}`)
      .catch((err) => toast("error", "无法打开 Modrinth", errorMessage(err)));
  };

  const openMcmod = () => {
    void api
      .openExternal(
        `https://search.mcmod.cn/s?key=${encodeURIComponent(hit.title)}`,
      )
      .catch((err) => toast("error", "无法打开 MC 百科", errorMessage(err)));
  };

  const copyName = async () => {
    try {
      await navigator.clipboard.writeText(hit.title);
      toast("success", "名称已复制", hit.title);
    } catch {
      toast("warning", "复制失败", "当前环境不允许访问剪贴板");
    }
  };

  const install = async (version: ModrinthVersionInfo) => {
    if (installing) return;
    setInstalling(version.id);
    setContentTask({
      title: `安装 ${hit.title}`,
      detail: `${version.versionNumber || version.name} · 正在下载`,
    });
    try {
      const report = await api.installModrinthVersion(targetVersionId, version.id);
      if (report.installed.length > 0) {
        toast(
          "success",
          "模组已安装",
          `${hit.title} ${version.versionNumber} · ${report.installed.length} 个文件`,
        );
      } else if (report.skipped.length > 0) {
        toast("info", "模组已经存在", `${hit.title} ${version.versionNumber}`);
      } else {
        toast("warning", "没有安装文件", hit.title);
      }
      if (report.warnings.length > 0) {
        toast("warning", "安装提示", report.warnings.slice(0, 2).join("；"));
      }
      if (report.installed.length > 0 || report.skipped.length > 0) onClose();
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

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={hit.title}
      description={`by ${hit.author || "未知作者"}`}
      size="lg"
      className="mod-detail-modal"
    >
      <div className="mod-detail-head">
        <span className="mod-detail-icon">
          <span>{hit.title.trim()[0]?.toUpperCase() ?? "?"}</span>
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
        <div className="mod-detail-copy">
          <p>{translatedDescription ?? hit.description}</p>
          <div className="mod-detail-meta">
            <span>{hit.downloads.toLocaleString("zh-CN")} 次下载</span>
            {hit.dateModified && <span>更新于 {formatRelative(hit.dateModified)}</span>}
            <span>Modrinth</span>
          </div>
          <div className="mod-detail-links">
            <button type="button" className="btn btn-sm btn-secondary" onClick={openModrinth}>
              <ExternalLink size={13} />
              转到 Modrinth
            </button>
            <button type="button" className="btn btn-sm btn-secondary" onClick={openMcmod}>
              <BookOpen size={13} />
              转到 MC 百科
            </button>
            <button type="button" className="btn btn-sm btn-ghost" onClick={() => void copyName()}>
              <Copy size={13} />
              复制名称
            </button>
          </div>
        </div>
      </div>

      {selectedVersion && selectedVersion.requiredDependencies > 0 && (
        <div className="mod-prereq">
          <div className="mod-prereq-head">
            <strong>前置资源</strong>
            <span>安装时自动补齐</span>
          </div>
          {dependencyIds.length > 0 ? (
            <div className="mod-prereq-list">
              {dependencyIds.map((id) => {
                const project = prerequisites[id];
                return (
                  <div className="mod-prereq-item" key={id}>
                    <span className="mod-prereq-icon">
                      {project?.iconUrl ? (
                        <img
                          src={project.iconUrl}
                          alt=""
                          loading="lazy"
                          referrerPolicy="no-referrer"
                        />
                      ) : (
                        <Package size={16} />
                      )}
                    </span>
                    <div>
                      <strong>{project?.title ?? "读取前置信息…"}</strong>
                      <span>{project?.slug || id}</span>
                    </div>
                  </div>
                );
              })}
            </div>
          ) : (
            <p>该版本需要 {selectedVersion.requiredDependencies} 个前置资源。</p>
          )}
        </div>
      )}

      <div className="mod-detail-versions-head">
        <strong>选择版本</strong>
        <span>仅显示可安装到 {targetLabel} 的版本</span>
      </div>

      {loading ? (
        <div className="loading-panel">
          <Loader2 size={18} className="spin" />
          正在读取版本…
        </div>
      ) : error ? (
        <div className="inline-alert warn">
          <AlertTriangle size={16} />
          <div>
            <strong>无法读取版本</strong>
            <span>{error}</span>
          </div>
        </div>
      ) : versions.length === 0 ? (
        <div className="table-empty">没有找到与当前版本兼容的文件。</div>
      ) : (
        <div className="mod-version-list">
          {versions.map((version) => (
            <div
              className={cx(
                "mod-version-row",
                version.id === selectedVersion?.id && "is-selected",
              )}
              key={version.id}
              role="button"
              tabIndex={0}
              onClick={() => setSelectedVersionId(version.id)}
              onKeyDown={(event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  setSelectedVersionId(version.id);
                }
              }}
            >
              <div className="mod-version-main">
                <div className="mod-version-title">
                  <strong>{version.versionNumber || version.name}</strong>
                  <span className={version.versionType === "release" ? "tag" : "tag tag-warn"}>
                    {TYPE_LABEL[version.versionType] ?? version.versionType}
                  </span>
                </div>
                <div className="mod-version-meta">
                  <span>
                    {version.gameVersions.slice(-3).join(" / ")} · {version.loaders.join(" / ")}
                  </span>
                  {version.datePublished && <span>{formatRelative(version.datePublished)}</span>}
                  {version.fileSize > 0 && <span>{formatBytes(version.fileSize)}</span>}
                  {version.requiredDependencies > 0 && (
                    <span>{version.requiredDependencies} 个必需依赖</span>
                  )}
                  {version.fileName && <span className="mod-version-file">{version.fileName}</span>}
                </div>
              </div>
              <button
                type="button"
                className="btn btn-sm btn-secondary"
                disabled={installing !== null && installing !== version.id}
                onClick={() => {
                  if (installing === version.id) {
                    void api.cancelModrinthInstall().catch(() => {});
                  } else {
                    void install(version);
                  }
                }}
              >
                {installing === version.id ? <X size={12} /> : <Download size={12} />}
                {installing === version.id ? "取消下载" : "安装"}
              </button>
            </div>
          ))}
        </div>
      )}
    </Modal>
  );
}
