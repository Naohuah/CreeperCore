export interface LibrarySkin {
  id: string;
  name: string;
  dataUrl: string;
  variant: "classic" | "slim";
  createdAt: string;
}

const STORAGE_KEY = "creeper-core-skin-library";
const MAX_SKINS = 24;

export function readSkinLibrary(): LibrarySkin[] {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const value = JSON.parse(raw) as unknown;
    if (!Array.isArray(value)) return [];
    return value
      .filter(
        (item): item is LibrarySkin =>
          Boolean(item) &&
          typeof item === "object" &&
          typeof (item as LibrarySkin).id === "string" &&
          typeof (item as LibrarySkin).dataUrl === "string" &&
          (item as LibrarySkin).dataUrl.startsWith("data:image/"),
      )
      .map((item): LibrarySkin => ({
        ...item,
        name: item.name || "未命名皮肤",
        variant: item.variant === "slim" ? "slim" : "classic",
        createdAt: item.createdAt || new Date().toISOString(),
      }))
      .slice(0, MAX_SKINS);
  } catch {
    return [];
  }
}

export function addSkinToLibrary(
  name: string,
  dataUrl: string,
  variant: "classic" | "slim",
): LibrarySkin[] {
  const current = readSkinLibrary();
  const existing = current.find((item) => item.dataUrl === dataUrl);
  if (existing) return current;
  const skin: LibrarySkin = {
    id: crypto.randomUUID(),
    name: name.trim() || "未命名皮肤",
    dataUrl,
    variant,
    createdAt: new Date().toISOString(),
  };
  const next = [skin, ...current].slice(0, MAX_SKINS);
  writeSkinLibrary(next);
  return next;
}

export function removeSkinFromLibrary(id: string): LibrarySkin[] {
  const next = readSkinLibrary().filter((item) => item.id !== id);
  writeSkinLibrary(next);
  return next;
}

function writeSkinLibrary(skins: LibrarySkin[]): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(skins));
  } catch {
    // The current session still has the updated list when storage is full.
  }
}
