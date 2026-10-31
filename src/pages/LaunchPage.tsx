import { useState } from "react";
import {
  ChevronDown,
  Download,
  Layers,
  Play,
  Settings2,
  UserPlus,
} from "lucide-react";
import { useAppStore } from "../store";
import {
  accountKindLabel,
  cx,
  loaderLabel,
  versionTitle,
} from "../lib/format";
import { CreeperMark } from "../components/CreeperMark";
import { SkinHead } from "../components/SkinHead";
import { useAccountSkin } from "../lib/useAccountSkin";
import { DiagnosticsDialog } from "../components/DiagnosticsDialog";
import { VersionSelectDialog } from "../components/VersionSelectDialog";
import { InstanceDialog } from "../components/InstanceDialog";

export function LaunchPage() {
  const boot = useAppStore((s) => s.boot);
  const game = useAppStore((s) => s.game);
  const navigate = useAppStore((s) => s.navigate);
  const launch = useAppStore((s) => s.launch);
  const showOverlay = useAppStore((s) => s.showOverlay);
  const setAccountDialogOpen = useAppStore((s) => s.setAccountDialogOpen);
  const appearance = useAppStore((s) => s.appearance);
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const account = boot?.config.accounts.find(
    (item) => item.id === boot.config.activeAccountId,
  );
  const skin = useAccountSkin(account);

  if (!boot) return null;

  const selected = boot.versions.find(
    (item) => item.id === boot.config.selectedVersion,
  );
  const diagnostics = boot.diagnostics;
  const issueCount =
    diagnostics?.items.filter(
      (item) => item.severity === "warn" || item.severity === "error",
    ).length ?? 0;
  const statusTone = diagnostics?.hasMissingFiles
    ? "error"
    : issueCount > 0
      ? "warn"
      : "ok";
  const statusText = diagnostics?.hasMissingFiles
    ? "需要补全核心文件"
    : issueCount > 0
      ? `${issueCount} 项运行提示`
      : "运行环境正常";

  const startGame = () => {
    if (!selected) {
      navigate("downloads");
      return;
    }
    if (game.running) {
      showOverlay();
      return;
    }
    if (!account) {
      setAccountDialogOpen(true);
      return;
    }
    void launch(selected.id);
  };

  if (!selected) {
    return (
      <div className="page launch-page launch-page-simple">
        <section className="launch-empty">
          <CreeperMark size={52} />
          <h2>还没有可启动的版本</h2>
          <p>先安装一个 Minecraft 版本，或者把游戏目录指向已有的 .minecraft。</p>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => navigate("downloads")}
          >
            <Download size={16} />
            安装版本
          </button>
        </section>
      </div>
    );
  }

  const hasBackground = Boolean(appearance.background);

  return (
    <div
      className={cx(
        "page launch-page launch-page-simple",
        hasBackground && "has-background",
      )}
    >
      <section className="launch-hero">
        <div className="launch-hero-avatar">
          {skin ? (
            <SkinHead skin={skin} size={78} label={account?.username ?? "玩家"} />
          ) : (
            <CreeperMark size={78} />
          )}
        </div>
        <h1>{account?.username ?? "未选择账户"}</h1>
        <span className="launch-hero-kind">
          {account ? accountKindLabel(account.kind) : "点击下方按钮创建账户"}
        </span>

        <button
          type="button"
          className="launch-hero-button"
          onClick={startGame}
          disabled={game.launching}
        >
          {!account ? (
            <UserPlus size={18} />
          ) : (
            <Play size={18} fill="currentColor" />
          )}
          {!account
            ? "创建账户"
            : game.running && game.ready
              ? "回到游戏"
              : game.launching || game.running
                ? "正在启动…"
                : "启动游戏"}
        </button>

        <span className="launch-hero-version">
          {versionTitle(selected.id, selected.inheritsFrom, selected.loader)}
        </span>

        <button
          type="button"
          className={cx("launch-status", `is-${statusTone}`)}
          onClick={() => setDiagnosticsOpen(true)}
        >
          <span className="inline-status-dot" />
          {statusText}
          <ChevronDown size={12} />
        </button>

      </section>

      <div className="launch-bottom">
        <button
          type="button"
          className="launch-bottom-button"
          onClick={() => setPickerOpen(true)}
        >
          <Layers size={17} />
          <span>
            <strong>版本选择</strong>
            <small>
              {versionTitle(selected.id, selected.inheritsFrom, selected.loader)}
            </small>
          </span>
          <ChevronDown size={13} />
        </button>
        <button
          type="button"
          className="launch-bottom-button"
          onClick={() => setSettingsOpen(true)}
        >
          <Settings2 size={17} />
          <span>
            <strong>版本设置</strong>
            <small>{loaderLabel(selected.loader)} · 内存 / Java</small>
          </span>
        </button>
      </div>

      <VersionSelectDialog open={pickerOpen} onClose={() => setPickerOpen(false)} />
      <InstanceDialog
        open={settingsOpen}
        versionId={selected.id}
        onClose={() => setSettingsOpen(false)}
      />

      <DiagnosticsDialog
        open={diagnosticsOpen}
        onClose={() => setDiagnosticsOpen(false)}
        onRepair={() => {
          setDiagnosticsOpen(false);
          void useAppStore.getState().repair(selected?.id);
        }}
      />
    </div>
  );
}
