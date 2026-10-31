import { useAppStore } from "../store";
import { closeWindow, minimizeWindow, toggleMaximizeWindow } from "../lib/bridge";
import { CreeperMark } from "./CreeperMark";

export function Titlebar() {
  const running = useAppStore((s) => s.game.running);
  const installRunning = useAppStore((s) => s.install?.running ?? false);
  const askConfirm = useAppStore((s) => s.askConfirm);

  const handleClose = async () => {
    if (installRunning) {
      const accepted = await askConfirm({
        title: "任务仍在进行",
        description: "关闭启动器会中断当前安装或修复任务。已经校验完成的文件会保留。",
        confirmLabel: "中断并退出",
        danger: true,
      });
      if (!accepted) return;
    }
    await closeWindow();
  };

  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="titlebar-brand" data-tauri-drag-region>
        <CreeperMark size={21} />
        <span className="brand-name">苦力怕启动器</span>
      </div>
      <div className="titlebar-tools">
        {running && <span className="titlebar-running">游戏运行中</span>}
        <div className="window-controls">
          <button
            type="button"
            className="window-button"
            aria-label="最小化"
            onClick={() => void minimizeWindow()}
          >
            <svg viewBox="0 0 10 10" width="10" height="10" aria-hidden="true">
              <path d="M2 5h6" stroke="currentColor" strokeWidth="1.2" />
            </svg>
          </button>
          <button
            type="button"
            className="window-button"
            aria-label="最大化"
            onClick={() => void toggleMaximizeWindow()}
          >
            <svg viewBox="0 0 10 10" width="10" height="10" aria-hidden="true">
              <rect
                x="2"
                y="2"
                width="6"
                height="6"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.1"
              />
            </svg>
          </button>
          <button
            type="button"
            className="window-button window-close"
            aria-label="关闭"
            onClick={() => void handleClose()}
          >
            <svg viewBox="0 0 10 10" width="10" height="10" aria-hidden="true">
              <path
                d="M2.2 2.2l5.6 5.6M7.8 2.2L2.2 7.8"
                stroke="currentColor"
                strokeWidth="1.2"
              />
            </svg>
          </button>
        </div>
      </div>
    </header>
  );
}
