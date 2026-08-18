import brandMarkUrl from "@/assets/codex-api-switcher-mark.svg";
import { cn } from "@/lib/utils";

interface AppBrandMarkProps {
  className?: string;
}

export function AppBrandMark({ className }: AppBrandMarkProps) {
  return (
    <img
      alt="Codex API Switcher"
      className={cn("h-full w-full object-contain", className)}
      draggable={false}
      src={brandMarkUrl}
    />
  );
}
