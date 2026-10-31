export function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

export function formatBytes(bytes: number | null | undefined): string {
  if (!bytes || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value >= 100 || unit === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unit]}`;
}

export function formatMemory(mb: number | null | undefined): string {
  if (!mb) return "—";
  if (mb >= 1024) {
    const gb = mb / 1024;
    return `${Number.isInteger(gb) ? gb.toFixed(0) : gb.toFixed(1)} GB`;
  }
  return `${mb} MB`;
}

export function formatDate(iso: string | null | undefined): string {
  if (!iso) return "未知";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "未知";
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(date);
}

export function formatDateTime(iso: string | null | undefined): string {
  if (!iso) return "未知";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "未知";
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

export function formatRelative(iso: string | null | undefined): string {
  if (!iso) return "从未";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "未知";
  const delta = Date.now() - date.getTime();
  if (delta < 45_000) return "刚刚";
  const minutes = Math.floor(delta / 60_000);
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days} 天前`;
  if (days < 30) return `${Math.floor(days / 7)} 周前`;
  return formatDate(iso);
}

export function formatDuration(seconds: number | null | undefined): string {
  if (!seconds || seconds < 0) return "00:00";
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  if (h > 0) {
    return `${h}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  }
  return `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
}

export function formatElapsed(startIso: string | null, nowMs = Date.now()): string {
  if (!startIso) return "00:00";
  const started = new Date(startIso).getTime();
  if (Number.isNaN(started)) return "00:00";
  return formatDuration(Math.max(0, (nowMs - started) / 1000));
}

export function shortPath(path: string | null | undefined, max = 54): string {
  if (!path) return "未设置";
  if (path.length <= max) return path;
  const parts = path.split(/[\\/]/);
  let result = parts[parts.length - 1] ?? path;
  for (let i = parts.length - 2; i >= 0; i--) {
    const next = `${parts[i]}\\${result}`;
    if (next.length > max - 2) break;
    result = next;
  }
  return `…\\${result}`;
}

export function accountKindLabel(kind: string | null | undefined): string {
  if (kind === "msa") return "Microsoft 账户";
  return "离线账户";
}

export function loaderLabel(loader: string | null | undefined): string {
  switch (loader) {
    case "fabric":
      return "Fabric";
    case "quilt":
      return "Quilt";
    case "forge":
      return "Forge";
    case "neoforge":
      return "NeoForge";
    case "vanilla":
      return "原版";
    case "unknown":
      return "未知";
    default:
      return loader ?? "原版";
  }
}

export function loaderClass(loader: string | null | undefined): string {
  switch (loader) {
    case "fabric":
      return "loader-fabric";
    case "quilt":
      return "loader-quilt";
    case "forge":
    case "neoforge":
      return "loader-forge";
    case "unknown":
      return "loader-unknown";
    default:
      return "loader-vanilla";
  }
}

export function baseGameVersion(versionId: string, inheritsFrom?: string | null): string {
  if (inheritsFrom) return versionId.replace(new RegExp(`.*${escapeRegExp(inheritsFrom)}$`), inheritsFrom);
  if (versionId.startsWith("fabric-loader-")) {
    const parts = versionId.split("-");
    return parts.slice(3).join("-");
  }
  return versionId;
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export function versionTitle(
  versionId: string,
  inheritsFrom?: string | null,
  loader?: string,
): string {
  if (loader && loader !== "vanilla" && loader !== "unknown" && inheritsFrom) {
    return `${inheritsFrom} · ${loaderLabel(loader)}`;
  }
  return versionId;
}

const CATEGORY_ZH: Record<string, string> = {
  optimization: "优化",
  shaders: "光影",
  library: "前置",
  utility: "工具",
  decoration: "装饰",
  worldgen: "世界生成",
  adventure: "冒险",
  magic: "魔法",
  technology: "科技",
  storage: "存储",
  equipment: "装备",
  mobs: "生物",
  management: "管理",
  "game-mechanics": "机制",
  transportation: "交通",
  food: "食物",
  gameplay: "玩法",
  api: "API",
};

export function categoryLabel(category: string): string {
  return CATEGORY_ZH[category] ?? category;
}

export function modInitial(name: string): string {
  const text = name.replace(/[^A-Za-z0-9\u4e00-\u9fa5]/g, "");
  return (text[0] ?? "?").toUpperCase();
}

export function modColor(name: string): string {
  let hash = 0;
  for (const char of name) hash = (hash * 31 + char.charCodeAt(0)) % 360;
  return `hsl(${hash} 58% 52%)`;
}

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message?: unknown }).message ?? error);
  }
  return String(error);
}

export function typeLabel(type: string): string {
  switch (type) {
    case "release":
      return "正式版";
    case "snapshot":
      return "快照";
    case "old_beta":
      return "Beta";
    case "old_alpha":
      return "Alpha";
    default:
      return type;
  }
}

export function plural(count: number, unit: string): string {
  return `${count} ${unit}`;
}
