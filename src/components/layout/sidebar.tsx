import { ChevronLeft, ChevronRight } from "lucide-react";

import type { AppRoute } from "@/store/app-shell-store";
import { navigationItems } from "@/components/layout/navigation";
import { cn } from "@/lib/utils";

interface SidebarProps {
  route: AppRoute;
  collapsed: boolean;
  onRouteChange: (route: AppRoute) => void;
  onToggle: () => void;
}

export function Sidebar({
  route,
  collapsed,
  onRouteChange,
  onToggle,
}: SidebarProps) {
  return (
    <aside
      className={cn(
        "relative flex h-full shrink-0 flex-col bg-transparent px-3 py-4 text-slate-700 transition-all duration-300",
        "w-full",
        collapsed ? "lg:w-[92px]" : "lg:w-[264px]"
      )}
    >
      <nav className="flex-1 space-y-1.5 mt-2">
        {navigationItems.map((item) => {
          const Icon = item.icon;
          const active = route === item.route;

          return (
            <button
              key={item.route}
              className={cn(
                "group flex h-10 w-full min-w-0 items-center gap-3 rounded-lg px-3 text-left text-sm font-medium transition-all duration-200",
                active
                  ? "bg-primary text-primary-foreground shadow-sm shadow-primary/20"
                  : "text-slate-600 hover:bg-slate-200/60 hover:text-slate-900",
                collapsed ? "lg:justify-center lg:px-0" : ""
              )}
              onClick={() => onRouteChange(item.route)}
              type="button"
              title={collapsed ? item.label : undefined}
            >
              <Icon className={cn("h-4 w-4 shrink-0", active ? "text-primary-foreground" : "text-slate-400 group-hover:text-slate-600")} />
              <span className={cn("truncate", collapsed ? "lg:hidden" : null)}>{item.label}</span>
            </button>
          );
        })}
      </nav>

      <div className="mt-auto pt-4 flex lg:justify-center">
        <button
          onClick={onToggle}
          className={cn(
            "flex h-8 items-center justify-center rounded-lg border border-slate-200 bg-white text-slate-500 shadow-sm transition-colors hover:bg-slate-50 hover:text-slate-900 w-full lg:w-8",
            collapsed && "rotate-180"
          )}
          title={collapsed ? "展开" : "收起"}
        >
          <ChevronLeft className="h-4 w-4" />
        </button>
      </div>
    </aside>
  );
}
