import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  Check,
  FolderOpen,
  Play,
  RotateCcw,
  Square,
  Terminal,
  X,
} from "lucide-react";
import { useAppStore } from "../store";
import { useNow } from "../lib/useNow";
import { cx, formatElapsed, loaderLabel, versionTitle } from "../lib/format";
import { api, minimizeWindow } from "../lib/bridge";
import { CreeperMark } from "./CreeperMark";

export function LaunchOverlay() {
  const game = useAppStore((s) => s.game);
  const boot = useAppStore((s) => s.boot);
  const stopGame = useAppStore((s) => s.stopGame);
  const hideOverlay = useAppStore((s) => s.hideOverlay);
  const dismissGameResult = useAppStore((s) => s.dismissGameResult);
  const launch = useAppStore((s) => s.launch);
  const askConfirm = useAppStore((s) => s.askConfirm);
  const toast = useAppStore((s) => s.toast);
  const now = useNow(game.overlayOpen && game.running);
  const logRef = useRef<HTMLDivElement>(null);
  const readyRef = useRef(false);
  const [celebrating, setCelebrating] = useState(false);

  useEffect(() => {
    const node = logRef.current;
    if (node) node.scrollTop = node.scrollHeight;
  }, [game.logs, game.overlayOpen]);

  useEffect(() => {
    const wasReady = readyRef.current;
    readyRef.current = game.ready;
    if (!game.overlayOpen) {
      setCelebrating(false);
      return;
    }
    if (!game.ready || game.result) {
      setCelebrating(false);
      return;
    }
    if (wasReady) return;
    setCelebrating(true);
    const timer = window.setTimeout(() => {
      setCelebrating(false);
      hideOverlay();
      if (useAppStore.getState().boot?.config.minimizeOnLaunch) {
        void minimizeWindow();
      }
    }, 1900);
    return () => window.clearTimeout(timer);
  }, [game.ready, game.overlayOpen, game.result, hideOverlay]);

  useEffect(() => {
    if (!game.overlayOpen) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (game.running) hideOverlay();
      else if (game.result) dismissGameResult();
    };
    document.addEventListener("keydown", onKey);
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = previous;
    };
  }, [game.overlayOpen, game.running, game.result, hideOverlay, dismissGameResult]);

  if (!game.overlayOpen) return null;

  const version = boot?.versions.find((item) => item.id === game.versionId);
  const title = version
    ? versionTitle(version.id, version.inheritsFrom, version.loader)
    : game.versionId ?? "Minecraft";
  const mode = game.result ? "result" : game.ready ? "running" : "launching";
  const showSuccess = celebrating && game.ready && !game.result;
  const isError = Boolean(game.result && game.result.level !== "info");
  const visibleLogs = game.logs.slice(-300);
  const activeAccount = boot?.config.accounts.find(
    (item) => item.id === boot.config.activeAccountId,
  );
  const loginLabel = activeAccount?.kind === "msa" ? "正版登录" : "离线登录";
  const logText = visibleLogs.map((item) => item.line.toLowerCase()).join("\n");
  let launchPercent = 8;
  let launchStep = "准备启动";
  if (game.running) {
    launchPercent = 24;
    launchStep = "启动 Java";
  }
  if (logText.includes("backend library")) {
    launchPercent = 38;
    launchStep = "初始化图形环境";
  }
  if (logText.includes("created window")) {
    launchPercent = 55;
    launchStep = "创建游戏窗口";
  }
  if (logText.includes("reloading resourcemanager")) {
    launchPercent = 70;
    launchStep = "加载资源包";
  }
  if (logText.includes("sound engine started")) {
    launchPercent = 84;
    launchStep = "初始化声音";
  }
  if (logText.includes("creating pipeline for dimension")) {
    launchPercent = 94;
    launchStep = "进入主界面";
  }
  if (game.ready) {
    launchPercent = 100;
    launchStep = "游戏已打开";
  }

  const openLogs = async () => {
    try {
      await api.openPath("logs", game.versionId);
    } catch (error) {
      toast("error", "无法打开日志目录", String(error));
    }
  };

  const requestStop = async () => {
    const accepted = await askConfirm({
      title: "结束当前游戏？",
      description: "未保存的游戏进度可能丢失。",
      confirmLabel: "结束游戏",
      danger: true,
    });
    if (accepted) await stopGame();
  };

  const close = () => {
    setCelebrating(false);
    if (mode === "result") dismissGameResult();
    else hideOverlay();
  };

  const restart = () => {
    const id = game.versionId;
    dismissGameResult();
    if (id) void launch(id);
  };

  return createPortal(
    <div className="modal-backdrop launch-backdrop" role="presentation">
      <section
        className={cx(
          "launch-modal",
          mode === "launching" && "is-launching",
          showSuccess && "is-success",
        )}
        role="dialog"
        aria-modal="true"
        aria-label="游戏运行状态"
      >
        <header className="launch-modal-head">
          <div className="launch-modal-title">
            <CreeperMark size={28} />
            <div>
              <strong>{title}</strong>
              <span>
                {showSuccess
                  ? "游戏已打开"
                  : mode === "running"
                    ? `运行中 · ${formatElapsed(game.startedAt, now)}`
                    : mode === "launching"
                      ? "正在打开游戏"
                      : game.result?.title ?? "游戏已退出"}
              </span>
            </div>
          </div>
          <div className="launch-modal-head-actions">
            {mode === "running" && <span className="live-badge">已进入游戏</span>}
            <button type="button" className="icon-button subtle" onClick={close} aria-label="关闭">
              <X size={17} />
            </button>
          </div>
        </header>

        {showSuccess ? (
          <div className="launch-success" aria-live="polite">
            <div className="success-burst">
              <CreeperMark size={58} />
              <span className="success-check">
                <Check size={15} strokeWidth={3} />
              </span>
            </div>
            <strong>游戏已打开</strong>
            <span className="launch-success-version">{title}</span>
            <small>可以关闭启动器，游戏会继续运行。</small>
          </div>
        ) : mode === "result" && game.result ? (
          <div className="launch-result">
            <div className={cx("result-summary", isError && "is-error")}>
              <strong>{game.result.title}</strong>
              <p>{game.result.summary}</p>
            </div>
            {game.result.suggestions.length > 0 && (
              <div className="result-suggestions">
                <span className="field-label">建议</span>
                <ul>
                  {game.result.suggestions.map((suggestion) => (
                    <li key={suggestion}>{suggestion}</li>
                  ))}
                </ul>
              </div>
            )}
            <div className="result-log">
              <div className="result-log-head">
                <span>
                  <Terminal size={13} /> 日志
                </span>
                <button type="button" className="text-button" onClick={() => void openLogs()}>
                  <FolderOpen size={13} />
                  打开目录
                </button>
              </div>
              <pre>
                {visibleLogs.length
                  ? visibleLogs
                      .slice(-10)
                      .map((item) => item.line)
                      .join("\n")
                  : game.result.logPath || "没有日志内容"}
              </pre>
            </div>
            <div className="launch-modal-actions">
              <button type="button" className="btn btn-secondary" onClick={() => void openLogs()}>
                <FolderOpen size={15} />
                打开日志
              </button>
              <button type="button" className="btn btn-primary" onClick={restart}>
                <RotateCcw size={15} />
                重新启动
              </button>
            </div>
          </div>
        ) : (
          <>
            {mode === "launching" && (
              <div className="launch-stage launch-stage-clean">
                <div className="launch-mark-wrap">
                  <span className="launch-mark-halo" />
                  <CreeperMark size={46} />
                </div>
                <div className="launch-stage-copy">
                  <strong>正在启动游戏</strong>
                  <span className="launch-stage-version">{title}</span>
                  <div
                    className="launch-progress"
                    role="progressbar"
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={launchPercent}
                  >
                    <i style={{ width: `${launchPercent}%` }} />
                  </div>
                  <div className="launch-detail-table">
                    <div>
                      <span>当前步骤</span>
                      <strong>{launchStep}</strong>
                    </div>
                    <div>
                      <span>登录方式</span>
                      <strong>{loginLabel}</strong>
                    </div>
                    <div>
                      <span>启动进度</span>
                      <strong>{launchPercent}%</strong>
                    </div>
                  </div>
                  <small className="launch-stage-hint">
                    检测到游戏窗口后才会显示完成
                  </small>
                </div>
              </div>
            )}
            {mode !== "launching" && (
              <div className="launch-modal-meta">
                <span>{loaderLabel(version?.loader)}</span>
                <span>{boot?.selectedJava?.major ? `Java ${boot.selectedJava.major}` : "Java 自动"}</span>
                <span>{formatMemoryLabel(boot?.config.memoryMb ?? 0)}</span>
              </div>
            )}
            {game.warnings.length > 0 && (
              <div className="launch-warning-line">{game.warnings.join(" · ")}</div>
            )}
            {mode !== "launching" && (
              <div className="launch-log" ref={logRef}>
                {visibleLogs.length === 0 ? (
                  <span className="launch-log-empty">等待游戏输出…</span>
                ) : (
                  visibleLogs.map((item, index) => (
                    <div
                      className={cx(
                        "launch-log-line",
                        item.stream === "stderr" && "is-error",
                        /ERROR|Exception/i.test(item.line) && "is-error",
                        /WARN/i.test(item.line) && "is-warn",
                      )}
                      key={`${item.timestamp}-${index}`}
                    >
                      {item.line}
                    </div>
                  ))
                )}
              </div>
            )}
            <div
              className={cx(
                "launch-modal-actions",
                mode === "launching" && "is-launch-actions",
              )}
            >
              {mode === "launching" ? (
                <button
                  type="button"
                  className="btn btn-secondary launch-cancel"
                  disabled={game.stopping}
                  onClick={() => {
                    if (game.running) void stopGame();
                    else hideOverlay();
                  }}
                >
                  {game.stopping ? "正在取消…" : "取消启动"}
                </button>
              ) : (
                <>
                  <button type="button" className="btn btn-ghost" onClick={hideOverlay}>
                    后台运行
                  </button>
                  <button
                    type="button"
                    className="btn btn-danger-soft"
                    disabled={game.stopping}
                    onClick={() => void requestStop()}
                  >
                    <Square size={14} fill="currentColor" />
                    {game.stopping ? "正在结束…" : "停止游戏"}
                  </button>
                </>
              )}
            </div>
          </>
        )}
      </section>
    </div>,
    document.body,
  );
}

function formatMemoryLabel(mb: number): string {
  if (mb >= 1024) return `${(mb / 1024).toFixed(mb % 1024 === 0 ? 0 : 1)} GB`;
  return `${mb} MB`;
}

export function GamePill() {
  const game = useAppStore((s) => s.game);
  const boot = useAppStore((s) => s.boot);
  const showOverlay = useAppStore((s) => s.showOverlay);
  const stopGame = useAppStore((s) => s.stopGame);
  const dismissGameResult = useAppStore((s) => s.dismissGameResult);
  const now = useNow(game.running && !game.overlayOpen);
  const ended = !game.running && Boolean(game.result) && !game.overlayOpen;
  if ((!game.running && !ended) || game.overlayOpen) return null;
  const version = boot?.versions.find((item) => item.id === game.versionId);
  return (
    <div className={cx("game-pill", ended && "is-ended")}>
      <button type="button" className="game-pill-main" onClick={showOverlay}>
        <CreeperMark size={24} />
        <span>
          <strong>
            {ended
              ? game.result?.title
              : version
                ? versionTitle(version.id, version.inheritsFrom, version.loader)
                : "游戏"}
          </strong>
          <small>
            {ended
              ? "查看运行日志"
              : game.ready
                ? `${formatElapsed(game.startedAt, now)} · 运行中`
                : "正在启动 · 等待游戏窗口"}
          </small>
        </span>
        <Play size={13} />
      </button>
      {game.running ? (
        <button
          type="button"
          className="game-pill-stop"
          aria-label="停止游戏"
          onClick={() => void stopGame()}
        >
          <Square size={11} fill="currentColor" />
        </button>
      ) : (
        <button
          type="button"
          className="game-pill-stop"
          aria-label="关闭"
          onClick={dismissGameResult}
        >
          <X size={12} />
        </button>
      )}
    </div>
  );
}
