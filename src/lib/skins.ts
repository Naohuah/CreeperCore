import type { AccountProfile } from "./types";

const LOCAL_PREFIX = "creeper-core-skin:";
const ONLINE_PREFIX = "creeper-core-online-skin:";
const CAPE_PREFIX = "creeper-core-cape:";

function key(prefix: string, account: AccountProfile): string {
  return `${prefix}${account.uuid || account.id}`;
}

export function readLocalSkin(account: AccountProfile): string | null {
  try {
    const value = window.localStorage.getItem(key(LOCAL_PREFIX, account));
    return value && value.startsWith("data:image/") ? value : null;
  } catch {
    return null;
  }
}

export function readOnlineSkin(account: AccountProfile): string | null {
  try {
    const value = window.localStorage.getItem(key(ONLINE_PREFIX, account));
    return value && value.startsWith("data:image/") ? value : null;
  } catch {
    return null;
  }
}

export function readCape(account: AccountProfile): string | null {
  try {
    const value = window.localStorage.getItem(key(CAPE_PREFIX, account));
    return value && value.startsWith("data:image/") ? value : null;
  } catch {
    return null;
  }
}

export function writeCape(account: AccountProfile, dataUrl: string | null): void {
  try {
    const storageKey = key(CAPE_PREFIX, account);
    if (dataUrl) window.localStorage.setItem(storageKey, dataUrl);
    else window.localStorage.removeItem(storageKey);
  } catch {
    // ignore
  }
}

export function writeLocalSkin(account: AccountProfile, dataUrl: string): void {
  try {
    window.localStorage.setItem(key(LOCAL_PREFIX, account), dataUrl);
  } catch {
    // The 3D preview still applies for this session when storage is full.
  }
}

export function removeLocalSkin(account: AccountProfile): void {
  try {
    window.localStorage.removeItem(key(LOCAL_PREFIX, account));
  } catch {
    // ignore
  }
}

export function writeOnlineSkin(account: AccountProfile, dataUrl: string | null): void {
  try {
    const storageKey = key(ONLINE_PREFIX, account);
    if (dataUrl) window.localStorage.setItem(storageKey, dataUrl);
    else window.localStorage.removeItem(storageKey);
  } catch {
    // ignore
  }
}
