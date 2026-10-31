import type { ReactNode } from "react";
import { cx } from "../lib/format";

interface PageHeaderProps {
  title: string;
  description?: string;
  actions?: ReactNode;
}

export function PageHeader({ title, description, actions }: PageHeaderProps) {
  return (
    <header className="page-header">
      <div className="page-heading">
        <h1>{title}</h1>
        {description && <p>{description}</p>}
      </div>
      {actions && <div className="page-actions">{actions}</div>}
    </header>
  );
}

interface LoaderBadgeProps {
  loader: string;
  version?: string | null;
  compact?: boolean;
}

export function LoaderBadge({ loader, version, compact = false }: LoaderBadgeProps) {
  const label =
    loader === "vanilla"
      ? "原版"
      : loader === "fabric"
        ? "Fabric"
        : loader === "quilt"
          ? "Quilt"
          : loader === "forge"
            ? "Forge"
            : loader === "neoforge"
              ? "NeoForge"
              : loader;
  return (
    <span className={cx("loader-badge", `loader-${loader}`, compact && "is-compact")}>
      <span className="loader-dot" />
      {label}
      {version && !compact && <span className="loader-version">{version}</span>}
    </span>
  );
}

interface StatusPillProps {
  tone?: "ok" | "warn" | "error" | "info" | "muted";
  children: ReactNode;
  pulse?: boolean;
}

export function StatusPill({ tone = "ok", children, pulse }: StatusPillProps) {
  return (
    <span className={cx("status-pill", `tone-${tone}`, pulse && "is-pulsing")}>
      <span className="status-pill-dot" />
      {children}
    </span>
  );
}

interface EmptyStateProps {
  art?: ReactNode;
  title: string;
  description: string;
  action?: ReactNode;
}

export function EmptyState({ art, title, description, action }: EmptyStateProps) {
  return (
    <div className="empty-state">
      <div className="empty-art">{art}</div>
      <h3>{title}</h3>
      <p>{description}</p>
      {action && <div className="empty-action">{action}</div>}
    </div>
  );
}

interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  label?: string;
}

export function Toggle({ checked, onChange, disabled, label }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label ?? "开关"}
      disabled={disabled}
      className={cx("toggle", checked && "is-on")}
      onClick={() => onChange(!checked)}
    >
      <span className="toggle-track">
        <span className="toggle-thumb" />
      </span>
    </button>
  );
}

interface ProgressBarProps {
  value: number;
  tone?: "green" | "red" | "gold";
  indeterminate?: boolean;
}

export function ProgressBar({ value, tone = "green", indeterminate }: ProgressBarProps) {
  return (
    <div className={cx("progress-track", `tone-${tone}`, indeterminate && "is-indeterminate")}>
      <div
        className="progress-fill"
        style={indeterminate ? undefined : { width: `${Math.max(0, Math.min(100, value))}%` }}
      />
    </div>
  );
}
