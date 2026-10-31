export type AccentKey = "green" | "blue" | "pink" | "orange" | "purple";

export interface Appearance {
  background: string | null;
  accent: AccentKey;
  dim: number;
}

export interface AccentPreset {
  key: AccentKey;
  label: string;
  color: string;
  dark: string;
  soft: string;
}

export const ACCENTS: AccentPreset[] = [
  { key: "green", label: "苦力怕绿", color: "#3f8f43", dark: "#2f6b2f", soft: "#edf6ea" },
  { key: "blue", label: "深海蓝", color: "#2f7fd1", dark: "#235f9d", soft: "#eaf3fc" },
  { key: "pink", label: "樱花粉", color: "#d65f9a", dark: "#a84376", soft: "#fbeaf3" },
  { key: "orange", label: "熔岩橙", color: "#d9822b", dark: "#a85f17", soft: "#fdf2e5" },
  { key: "purple", label: "紫水晶", color: "#8a63c7", dark: "#6847a0", soft: "#f2edfb" },
];

const STORAGE_KEY = "creeper-core-appearance";

export const DEFAULT_APPEARANCE: Appearance = {
  background: null,
  accent: "green",
  dim: 0.48,
};

export function accentPreset(key: AccentKey): AccentPreset {
  return ACCENTS.find((preset) => preset.key === key) ?? ACCENTS[0];
}

export function readAppearance(): Appearance {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_APPEARANCE;
    const value = JSON.parse(raw) as Partial<Appearance>;
    const accent = ACCENTS.some((preset) => preset.key === value.accent)
      ? (value.accent as AccentKey)
      : "green";
    return {
      background:
        typeof value.background === "string" && value.background.startsWith("data:image/")
          ? value.background
          : null,
      accent,
      dim:
        typeof value.dim === "number" && Number.isFinite(value.dim)
          ? Math.max(0, Math.min(0.8, value.dim))
          : DEFAULT_APPEARANCE.dim,
    };
  } catch {
    return DEFAULT_APPEARANCE;
  }
}

export function applyAppearance(appearance: Appearance): void {
  if (typeof document === "undefined") return;
  const preset = accentPreset(appearance.accent);
  const root = document.documentElement;
  root.style.setProperty("--green", preset.color);
  root.style.setProperty("--green-dark", preset.dark);
  root.style.setProperty("--green-soft", preset.soft);
}

export function saveAppearance(appearance: Appearance): void {
  applyAppearance(appearance);
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(appearance));
  } catch {
    // The theme still applies for this session when storage is full.
  }
}

export async function prepareBackground(file: File): Promise<string> {
  if (!file.type.startsWith("image/")) {
    throw new Error("请选择图片文件");
  }
  if (file.size > 12 * 1024 * 1024) {
    throw new Error("背景图片不能超过 12 MB");
  }
  const raw = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error ?? new Error("无法读取图片"));
    reader.readAsDataURL(file);
  });
  const image = await new Promise<HTMLImageElement>((resolve, reject) => {
    const element = new Image();
    element.onload = () => resolve(element);
    element.onerror = () => reject(new Error("图片无法解析"));
    element.src = raw;
  });
  const maxWidth = 1920;
  const maxHeight = 1080;
  const scale = Math.min(1, maxWidth / image.width, maxHeight / image.height);
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(image.width * scale));
  canvas.height = Math.max(1, Math.round(image.height * scale));
  const context = canvas.getContext("2d");
  if (!context) throw new Error("无法处理图片");
  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  const dataUrl = canvas.toDataURL("image/jpeg", 0.82);
  if (dataUrl.length > 3_500_000) {
    throw new Error("压缩后的背景仍然过大，请换一张图片");
  }
  return dataUrl;
}
