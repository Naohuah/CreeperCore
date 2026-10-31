import { create } from "zustand";
import { api, listenEvent } from "./lib/bridge";
import {
  applyAppearance,
  readAppearance,
  saveAppearance,
  type Appearance,
} from "./lib/appearance";
import { errorMessage, versionTitle } from "./lib/format";
import type {
  BootstrapData,
  CrashReport,
  GameLogPayload,
  GameStartPayload,
  InstallProgress,
  InstallStartPayload,
  PageId,
} from "./lib/types";

export type DownloadSection =
  | "game"
  | "mods"
  | "packs"
  | "resourcepack"
  | "shader"
  | "datapack";

export const PAGES: PageId[] = [
  "launch",
  "versions",
  "downloads",
  "mods",
  "backups",
  "settings",
];

export function pageFromHash(): PageId {
  const raw = window.location.hash.replace(/^#\/?/, "").split("?")[0];
  if (raw === "home") return "launch";
  if (raw === "about") return "settings";
  return (PAGES as string[]).includes(raw) ? (raw as PageId) : "launch";
}

export type ToastKind = "info" | "success" | "warning" | "error";

export interface ToastMessage {
  id: number;
  kind: ToastKind;
  title: string;
  detail?: string;
}

export interface ConfirmOptions {
  title: string;
  description: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
}

interface ConfirmState extends ConfirmOptions {
  resolve: (value: boolean) => void;
}

export interface GameLogLine {
  line: string;
  stream: string;
  timestamp: string;
}

export interface InstallUiState {
  taskId: string | null;
  versionId: string;
  loader: string;
  progress: InstallProgress | null;
  running: boolean;
  error: string | null;
}

export interface ContentTask {
  title: string;
  detail: string;
}

export interface GameUiState {
  launching: boolean;
  running: boolean;
  ready: boolean;
  stopping: boolean;
  pid: number | null;
  versionId: string | null;
  startedAt: string | null;
  logPath: string | null;
  logs: GameLogLine[];
  result: CrashReport | null;
  overlayOpen: boolean;
  collapsed: boolean;
  warnings: string[];
}

interface AppStore {
  route: PageId;
  downloadSection: DownloadSection;
  boot: BootstrapData | null;
  loading: boolean;
  refreshing: boolean;
  bootError: string | null;
  toasts: ToastMessage[];
  confirmDialog: ConfirmState | null;
  accountDialogOpen: boolean;
  appearance: Appearance;
  install: InstallUiState | null;
  contentTask: ContentTask | null;
  game: GameUiState;
  initialize: () => Promise<void>;
  setAppearance: (patch: Partial<Appearance>) => void;
  setContentTask: (task: ContentTask | null) => void;
  refresh: (quiet?: boolean) => Promise<void>;
  navigate: (page: PageId) => void;
  setDownloadSection: (section: DownloadSection) => void;
  openDownloads: (section: DownloadSection) => void;
  toast: (kind: ToastKind, title: string, detail?: string) => void;
  dismissToast: (id: number) => void;
  askConfirm: (options: ConfirmOptions) => Promise<boolean>;
  finishConfirm: (value: boolean) => void;
  setAccountDialogOpen: (open: boolean) => void;
  selectVersion: (versionId: string) => Promise<void>;
  startInstall: (
    versionId: string,
    loader: "vanilla" | "fabric" | "forge" | "neoforge",
  ) => Promise<void>;
  cancelInstall: () => Promise<void>;
  repair: (versionId?: string) => Promise<void>;
  launch: (versionId?: string) => Promise<void>;
  stopGame: () => Promise<void>;
  showOverlay: () => void;
  hideOverlay: () => void;
  dismissGameResult: () => void;
  noteGameStarted: (payload: GameStartPayload, warnings?: string[]) => void;
  noteGameLog: (payload: GameLogPayload) => void;
  noteGameExit: (report: CrashReport) => void;
}

const emptyGame: GameUiState = {
  launching: false,
  running: false,
  ready: false,
  stopping: false,
  pid: null,
  versionId: null,
  startedAt: null,
  logPath: null,
  logs: [],
  result: null,
  overlayOpen: false,
  collapsed: false,
  warnings: [],
};

let toastSeq = 0;
const initialAppearance = readAppearance();
applyAppearance(initialAppearance);

export const useAppStore = create<AppStore>((set, get) => ({
  route: pageFromHash(),
  downloadSection: "game",
  boot: null,
  loading: true,
  refreshing: false,
  bootError: null,
  toasts: [],
  confirmDialog: null,
  accountDialogOpen: false,
  appearance: initialAppearance,
  install: null,
  contentTask: null,
  game: { ...emptyGame },

  setContentTask: (task) => set({ contentTask: task }),

  setAppearance: (patch) => {
    const appearance = { ...get().appearance, ...patch };
    saveAppearance(appearance);
    set({ appearance });
  },

  initialize: async () => {
    try {
      await connectEvents();
    } catch (error) {
      console.error("event bridge failed", error);
    }
    await get().refresh();
  },

  refresh: async (quiet = false) => {
    if (quiet) {
      set({ refreshing: true });
    } else {
      set({ loading: get().boot === null, bootError: null });
    }
    try {
      const boot = await api.bootstrap();
      const previous = get().game;
      let game = previous;
      if (boot.gameStatus) {
        game = {
          ...previous,
          running: true,
          ready: true,
          launching: false,
          pid: boot.gameStatus.pid,
          versionId: boot.gameStatus.versionId,
          startedAt: boot.gameStatus.startedAt,
          logPath: boot.gameStatus.logPath,
          logs:
            previous.logs.length > 0
              ? previous.logs
              : boot.logs.map((line) => ({
                  line,
                  stream: "stdout",
                  timestamp: new Date().toISOString(),
                })),
        };
      } else if (!previous.launching) {
        game = {
          ...previous,
          running: false,
          ready: false,
          launching: false,
          pid: null,
          versionId: previous.result ? previous.versionId : null,
          startedAt: previous.result ? previous.startedAt : null,
          logPath: previous.result ? previous.logPath : null,
          logs: previous.result ? previous.logs : [],
        };
      }
      let install = get().install;
      if (boot.activeInstall && !install?.running) {
        install = {
          taskId: boot.activeInstall.taskId,
          versionId: boot.config.selectedVersion ?? "",
          loader: "unknown",
          progress: null,
          running: true,
          error: null,
        };
      } else if (!boot.activeInstall && install?.running) {
        install = { ...install, running: false };
      }
      set({
        boot,
        bootError: null,
        loading: false,
        refreshing: false,
        game,
        install,
      });
    } catch (error) {
      set({
        loading: false,
        refreshing: false,
        bootError: get().boot ? null : errorMessage(error),
      });
      if (quiet || get().boot) {
        get().toast("error", "刷新失败", errorMessage(error));
      }
    }
  },

  navigate: (page) => {
    if (get().route === page) return;
    window.location.hash = `#/${page}`;
    set({ route: page });
  },

  setDownloadSection: (section) => set({ downloadSection: section }),

  openDownloads: (section) => {
    set({ downloadSection: section });
    get().navigate("downloads");
  },

  toast: (kind, title, detail) => {
    const id = ++toastSeq;
    set((state) => ({
      toasts: [...state.toasts.slice(-3), { id, kind, title, detail }],
    }));
    window.setTimeout(() => {
      set((state) => ({ toasts: state.toasts.filter((item) => item.id !== id) }));
    }, kind === "error" ? 6500 : 4500);
  },

  dismissToast: (id) => {
    set((state) => ({ toasts: state.toasts.filter((item) => item.id !== id) }));
  },

  askConfirm: (options) =>
    new Promise<boolean>((resolve) => {
      set({ confirmDialog: { ...options, resolve } });
    }),

  finishConfirm: (value) => {
    const dialog = get().confirmDialog;
    if (dialog) dialog.resolve(value);
    set({ confirmDialog: null });
  },

  setAccountDialogOpen: (open) => set({ accountDialogOpen: open }),

  selectVersion: async (versionId) => {
    try {
      await api.selectVersion(versionId);
      await get().refresh(true);
      get().toast("success", "已切换版本", versionId);
    } catch (error) {
      get().toast("error", "切换版本失败", errorMessage(error));
    }
  },

  startInstall: async (versionId, loader) => {
    const current = get().install;
    if (current?.running) {
      get().toast("warning", "已有任务正在进行", "请等待当前安装或修复完成");
      return;
    }
    set({
      install: {
        taskId: null,
        versionId,
        loader,
        progress: null,
        running: true,
        error: null,
      },
    });
    try {
      await api.installGame(versionId, loader, true);
      set((state) => ({
        install: state.install
          ? { ...state.install, running: false, progress: state.install.progress }
          : null,
      }));
      await get().refresh(true);
    } catch (error) {
      const message = errorMessage(error);
      const alreadyNotified = get().install?.error === message;
      set((state) => ({
        install: state.install
          ? { ...state.install, running: false, error: message }
          : null,
      }));
      if (!alreadyNotified && !message.includes("已取消")) {
        get().toast("error", "安装失败", message);
      }
    }
  },

  cancelInstall: async () => {
    try {
      await api.cancelInstall();
      get().toast("info", "正在取消", "已向下载管线发送中止信号");
    } catch (error) {
      get().toast("error", "取消失败", errorMessage(error));
    }
  },

  repair: async (versionId) => {
    const boot = get().boot;
    const target = versionId ?? boot?.config.selectedVersion;
    if (!target) {
      get().toast("warning", "还没有选择版本", "先安装或选择一个版本再修复");
      return;
    }
    if (get().install?.running) {
      get().toast("warning", "已有任务正在进行");
      return;
    }
    set({
      install: {
        taskId: null,
        versionId: target,
        loader: "repair",
        progress: null,
        running: true,
        error: null,
      },
    });
    try {
      await api.repairGame(target);
      await get().refresh(true);
      get().toast("success", "核心校验完成", "缺失文件已补全，索引已刷新");
    } catch (error) {
      const message = errorMessage(error);
      set((state) => ({
        install: state.install
          ? { ...state.install, running: false, error: message }
          : null,
      }));
      get().toast("error", "修复未完成", message);
    }
  },

  launch: async (versionId) => {
    const boot = get().boot;
    if (!boot) return;
    const target = versionId ?? boot.config.selectedVersion;
    if (!target) {
      get().toast("warning", "没有可启动的版本", "先在版本库中安装一个版本");
      return;
    }
    if (get().game.running || get().game.launching) {
      get().toast("info", "游戏已经在运行", "双击苦力怕也无法叠加一只");
      set((state) => ({ game: { ...state.game, overlayOpen: true } }));
      return;
    }
    set((state) => ({
      game: {
        ...state.game,
        launching: true,
        overlayOpen: true,
        collapsed: false,
        result: null,
        warnings: [],
        versionId: target,
        logs: [
          {
            line: "正在构建 Creeper Core 启动链…",
            stream: "system",
            timestamp: new Date().toISOString(),
          },
        ],
      },
    }));
    try {
      const result = await api.launchGame(target);
      set((state) => ({
        game: {
          ...state.game,
          launching: false,
          running: true,
          ready: false,
          stopping: false,
          pid: result.pid,
          versionId: result.versionId,
          startedAt: result.startedAt,
          logPath: result.logPath,
          warnings: result.warnings,
        },
      }));

    } catch (error) {
      const message = errorMessage(error);
      set((state) => ({
        game: {
          ...state.game,
          launching: false,
          running: false,
          ready: false,
          result: {
            level: "error",
            kind: "launch",
            exitCode: null,
            title: "游戏启动失败",
            summary: message,
            suggestions: [
              "打开「运行检查」检查 Java、核心文件与模组依赖",
              "尝试在设置中选择版本匹配的 Java",
            ],
            logPath: state.game.logPath ?? "",
            signalLine: null,
            playTimeSecs: 0,
          },
        },
      }));
      get().toast("error", "启动失败", message);
    }
  },

  stopGame: async () => {
    if (!get().game.running) return;
    set((state) => ({ game: { ...state.game, stopping: true } }));
    try {
      await api.stopGame();
    } catch (error) {
      get().toast("error", "终止失败", errorMessage(error));
      set((state) => ({ game: { ...state.game, stopping: false } }));
    }
  },

  showOverlay: () => set((state) => ({ game: { ...state.game, overlayOpen: true } })),
  hideOverlay: () =>
    set((state) => ({ game: { ...state.game, overlayOpen: false, collapsed: state.game.running } })),
  dismissGameResult: () =>
    set((state) => ({
      game: {
        ...state.game,
        overlayOpen: false,
        collapsed: false,
        result: null,
        logs: state.game.running ? state.game.logs : [],
      },
    })),

  noteGameStarted: (payload, warnings = []) => {
    set((state) => ({
      game: {
        ...state.game,
        launching: false,
        running: true,
        ready: false,
        stopping: false,
        overlayOpen: true,
        pid: payload.pid,
        versionId: payload.versionId,
        startedAt: payload.startedAt,
        logPath: payload.logPath,
        warnings,
      },
    }));
  },

  noteGameLog: (payload) => {
    set((state) => ({
      game: {
        ...state.game,
        logs: [
          ...state.game.logs.slice(-1200),
          {
            line: payload.line,
            stream: payload.stream,
            timestamp: payload.timestamp,
          },
        ],
      },
    }));
  },

  noteGameExit: (report) => {
    const showPanel = get().boot?.config.showLogOnCrash ?? true;
    set((state) => ({
      game: {
        ...state.game,
        launching: false,
        running: false,
        ready: false,
        stopping: false,
        pid: null,
        result: report,
        overlayOpen: showPanel,
        collapsed: false,
      },
    }));
  },
}));

let eventsConnected: Promise<void> | null = null;

async function connectEvents(): Promise<void> {
  if (eventsConnected) return eventsConnected;
  eventsConnected = (async () => {
    const store = useAppStore.getState;
    await listenEvent<InstallStartPayload>("install-start", (payload) => {
      useAppStore.setState({
        install: {
          taskId: payload.taskId,
          versionId: payload.versionId,
          loader: payload.loader,
          progress: null,
          running: true,
          error: null,
        },
      });
    });
    await listenEvent<InstallProgress>("install-progress", (payload) => {
      const state = store();
      useAppStore.setState({
        install: {
          taskId: payload.taskId,
          versionId: state.install?.versionId ?? "",
          loader: state.install?.loader ?? "unknown",
          progress: payload,
          running: true,
          error: null,
        },
      });
    });
    await listenEvent<{ versionId?: string }>("install-finish", (payload) => {
      useAppStore.setState((state) => ({
        install: state.install
          ? { ...state.install, running: false, error: null }
          : null,
      }));
      store().toast(
        "success",
        "安装完成",
        payload?.versionId ? `${payload.versionId} 已加入版本库` : "文件已就绪",
      );
      window.setTimeout(() => {
        void store().refresh(true);
      }, 600);
    });
    await listenEvent<string>("install-error", (message) => {
      useAppStore.setState((state) => ({
        install: state.install
          ? { ...state.install, running: false, error: message }
          : null,
      }));
      if (!message.includes("已取消")) {
        store().toast("error", "任务中断", message);
      }
    });
    await listenEvent<GameStartPayload & { warnings?: string[] }>("game-start", (payload) => {
      store().noteGameStarted(payload, payload.warnings ?? []);
    });
    await listenEvent<{ pid: number; versionId: string }>("game-ready", (payload) => {
      const state = store();
      const version = state.boot?.versions.find((item) => item.id === payload.versionId);
      const label = version
        ? versionTitle(version.id, version.inheritsFrom, version.loader)
        : payload.versionId;
      useAppStore.setState((current) => ({
        game: { ...current.game, ready: true },
      }));
      if (!state.game.overlayOpen) {
        state.toast("success", "游戏已打开", label);
      }
    });
    await listenEvent<GameLogPayload>("game-log", (payload) => {
      store().noteGameLog(payload);
    });
    await listenEvent<CrashReport>("game-exit", (report) => {
      store().noteGameExit(report);
      if (report.level === "info") {
        store().toast("info", report.title, report.summary);
      } else {
        store().toast("error", report.title, report.summary);
      }
    });
    await listenEvent<{ pid: number }>("game-stopping", () => {
      useAppStore.setState((state) => ({
        game: { ...state.game, stopping: true },
      }));
    });
  })();
  return eventsConnected;
}
