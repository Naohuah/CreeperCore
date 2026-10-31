import { useEffect, useRef, type CSSProperties, type JSX } from "react";
import { cx } from "./lib/format";
import { useAppStore } from "./store";
import { Titlebar } from "./components/Titlebar";
import { Sidebar } from "./components/Sidebar";
import { Toasts } from "./components/Toasts";
import { AccountDialog } from "./components/AccountDialog";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { InstallDock } from "./components/InstallDock";
import { GamePill, LaunchOverlay } from "./components/LaunchOverlay";
import { BootError, Splash } from "./components/Splash";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { LaunchPage } from "./pages/LaunchPage";
import { VersionsPage } from "./pages/VersionsPage";
import { DownloadsPage } from "./pages/DownloadsPage";
import { ModsPage } from "./pages/ModsPage";
import { BackupsPage } from "./pages/BackupsPage";
import { SettingsPage } from "./pages/SettingsPage";
import type { PageId } from "./lib/types";

const PAGE_COMPONENT: Record<PageId, () => JSX.Element | null> = {
  launch: LaunchPage,
  versions: VersionsPage,
  downloads: DownloadsPage,
  mods: ModsPage,
  backups: BackupsPage,
  settings: SettingsPage,
};

const PAGE_TITLE: Record<PageId, string> = {
  launch: "启动游戏",
  versions: "版本管理",
  downloads: "下载游戏",
  mods: "模组管理",
  backups: "备份与恢复",
  settings: "设置",
};

const NAV_SHORTCUTS: Record<string, PageId> = {
  "1": "launch",
  "2": "versions",
  "3": "downloads",
  "4": "mods",
  "5": "backups",
  "6": "settings",
};

export default function App() {
  const boot = useAppStore((s) => s.boot);
  const bootError = useAppStore((s) => s.bootError);
  const loading = useAppStore((s) => s.loading);
  const route = useAppStore((s) => s.route);
  const game = useAppStore((s) => s.game);
  const initialize = useAppStore((s) => s.initialize);
  const navigate = useAppStore((s) => s.navigate);
  const launch = useAppStore((s) => s.launch);
  const showOverlay = useAppStore((s) => s.showOverlay);
  const appearance = useAppStore((s) => s.appearance);
  const mainRef = useRef<HTMLElement>(null);

  useEffect(() => {
    void initialize();
  }, [initialize]);

  useEffect(() => {
    const onHash = () => {
      const raw = window.location.hash.replace(/^#\/?/, "").split("?")[0];
      const valid: PageId[] = [
        "launch",
        "versions",
        "downloads",
        "mods",
        "backups",
        "settings",
      ];
      const page: PageId =
        raw === "home"
          ? "launch"
          : raw === "about"
            ? "settings"
            : valid.includes(raw as PageId)
              ? (raw as PageId)
              : "launch";
      useAppStore.setState({ route: page });
    };
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);

  useEffect(() => {
    document.title = `${PAGE_TITLE[route]} · 苦力怕启动器`;
    mainRef.current?.scrollTo({ top: 0, behavior: "auto" });
  }, [route]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
        event.preventDefault();
        if (!boot) return;
        if (game.running) showOverlay();
        else void launch();
      }
      if (event.ctrlKey && NAV_SHORTCUTS[event.key]) {
        const target = event.target as HTMLElement | null;
        if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
        event.preventDefault();
        navigate(NAV_SHORTCUTS[event.key]);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [boot, game.running, launch, navigate, showOverlay]);

  if (!boot && loading) return <Splash />;
  if (!boot && bootError) {
    return (
      <div className="app boot-state">
        <BootError
          message={bootError}
          onRetry={() => {
            useAppStore.setState({ loading: true, bootError: null });
            void initialize();
          }}
        />
      </div>
    );
  }
  if (!boot) return <Splash />;

  const Page = PAGE_COMPONENT[route];
  const wallpaper = route === "launch" ? appearance.background : null;
  const appStyle = wallpaper
    ? ({
        backgroundImage: `url(${wallpaper})`,
        "--wallpaper-dim": appearance.dim,
      } as CSSProperties)
    : undefined;

  return (
    <div className={cx("app", wallpaper && "has-wallpaper")} style={appStyle}>
      <Titlebar />
      <div className="app-shell">
        <Sidebar />
        <main className="main" ref={mainRef}>
          <div className="page-stage" key={route}>
            <ErrorBoundary resetKey={route}>
              <Page />
            </ErrorBoundary>
          </div>
        </main>
      </div>
      <Toasts />
      <AccountDialog />
      <ConfirmDialog />
      <LaunchOverlay />
      <InstallDock />
      <GamePill />
    </div>
  );
}
