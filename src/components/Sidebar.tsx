import {
  Archive,
  Download,
  Layers3,
  Package,
  Play,
  Settings2,
} from "lucide-react";
import { useAppStore } from "../store";
import { CreeperMark } from "./CreeperMark";
import { SkinHead } from "./SkinHead";
import { SkinPreview } from "./SkinPreview";
import { accountKindLabel } from "../lib/format";
import { useAccountSkin } from "../lib/useAccountSkin";
import type { PageId } from "../lib/types";

interface NavItem {
  id: PageId;
  label: string;
  icon: typeof Play;
}

const NAV: NavItem[] = [
  { id: "launch", label: "启动", icon: Play },
  { id: "versions", label: "版本", icon: Layers3 },
  { id: "downloads", label: "下载", icon: Download },
  { id: "mods", label: "模组", icon: Package },
  { id: "backups", label: "备份", icon: Archive },
  { id: "settings", label: "设置", icon: Settings2 },
];

export function Sidebar() {
  const route = useAppStore((s) => s.route);
  const navigate = useAppStore((s) => s.navigate);
  const boot = useAppStore((s) => s.boot);
  const setAccountDialogOpen = useAppStore((s) => s.setAccountDialogOpen);
  const game = useAppStore((s) => s.game);

  const account = boot?.config.accounts.find(
    (item) => item.id === boot.config.activeAccountId,
  );
  const skin = useAccountSkin(account);

  return (
    <aside className="sidebar">
      <nav className="sidebar-nav" aria-label="主导航">
        {NAV.map((item) => {
          const Icon = item.icon;
          const active = route === item.id;
          return (
            <button
              key={item.id}
              type="button"
              className={`nav-item nav-${item.id} ${active ? "is-active" : ""}`}
              onClick={() => navigate(item.id)}
              aria-current={active ? "page" : undefined}
            >
              <Icon size={17} strokeWidth={1.9} />
              <span>{item.label}</span>
              {item.id === "launch" && game.running && <i className="nav-live-dot" />}
            </button>
          );
        })}
      </nav>

      <div className="sidebar-bottom">
        <button
          type="button"
          className="account-card"
          onClick={() => setAccountDialogOpen(true)}
          aria-label="切换玩家账户"
        >
          {skin ? (
            <SkinHead skin={skin} size={28} label={account?.username ?? "玩家"} />
          ) : (
            <CreeperMark size={28} />
          )}
          <span className="account-copy">
            <strong>{account?.username ?? "未选择账户"}</strong>
            <small>{account ? accountKindLabel(account.kind) : "未选择"}</small>
          </span>
          <span className="account-caret">›</span>
        </button>
        <span className="sidebar-version">Creeper Core 0.1.0</span>
      </div>
      {account && <SkinPreview account={account} />}
    </aside>
  );
}
