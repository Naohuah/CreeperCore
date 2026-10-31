import { CreeperMark } from "./CreeperMark";

export function Splash() {
  return (
    <div className="splash">
      <div className="splash-content">
        <div className="splash-mark">
          <CreeperMark size={72} halo />
        </div>
        <h1 className="splash-wordmark">苦力怕启动器</h1>
        <span className="splash-sub">CREEPER CORE · 正在加载本地核心</span>
        <div className="splash-progress">
          <span />
        </div>
      </div>
    </div>
  );
}

export function BootError({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <div className="boot-error">
      <CreeperMark size={64} muted />
      <h1>启动器没有响应</h1>
      <p>{message}</p>
      <button type="button" className="btn btn-primary" onClick={onRetry}>
        重试
      </button>
      <span className="boot-error-hint">
        如果问题持续，请确认 WebView2 正常，并查看启动器的终端输出。
      </span>
    </div>
  );
}
