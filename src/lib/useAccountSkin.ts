import { useEffect, useState } from "react";
import type { AccountProfile } from "./types";
import { readLocalSkin, readOnlineSkin } from "./skins";

export const SKIN_CHANGED_EVENT = "creeper-skin-changed";

export function accountSkin(account: AccountProfile | undefined): string | null {
  if (!account) return null;
  return (
    readLocalSkin(account) ??
    (account.kind === "msa" ? readOnlineSkin(account) : null)
  );
}

/** Read the account's current head texture and update when it is replaced. */
export function useAccountSkin(account: AccountProfile | undefined): string | null {
  const [skin, setSkin] = useState(() => accountSkin(account));
  useEffect(() => {
    const update = () => setSkin(accountSkin(account));
    update();
    window.addEventListener(SKIN_CHANGED_EVENT, update);
    return () => window.removeEventListener(SKIN_CHANGED_EVENT, update);
  }, [account]);
  return skin;
}

export function notifySkinChanged(): void {
  window.dispatchEvent(new Event(SKIN_CHANGED_EVENT));
}
