import { Component, type ErrorInfo, type ReactNode } from "react";
import { AlertTriangle, RotateCcw } from "lucide-react";

interface ErrorBoundaryProps {
  children: ReactNode;
  /** When this changes, a previously crashed subtree gets another chance. */
  resetKey?: string;
  fullWindow?: boolean;
}

interface ErrorBoundaryState {
  error: Error | null;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("CreeperCore UI error", error, info.componentStack);
    try {
      window.localStorage.setItem(
        "creeper-last-ui-error",
        `${new Date().toISOString()}\n${error.message}\n${error.stack ?? ""}`,
      );
    } catch {
      // A crash report must never cause another crash.
    }
  }

  componentDidUpdate(previous: ErrorBoundaryProps) {
    if (previous.resetKey !== this.props.resetKey && this.state.error) {
      this.setState({ error: null });
    }
  }

  render() {
    if (!this.state.error) return this.props.children;
    const error = this.state.error;
    return (
      <div className={this.props.fullWindow ? "root-crash" : "page-crash"}>
        <div className="crash-card">
          <span className="crash-icon">
            <AlertTriangle size={22} />
          </span>
          <strong>这个页面加载出错了</strong>
          <span className="crash-message">{error.message || "未知错误"}</span>
          <p>
            你的账户、版本和下载数据没有丢失。可以重试这个页面，或者重新加载启动器。
          </p>
          <div className="crash-actions">
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => this.setState({ error: null })}
            >
              <RotateCcw size={14} />
              重试
            </button>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => window.location.reload()}
            >
              重新加载启动器
            </button>
          </div>
        </div>
      </div>
    );
  }
}
