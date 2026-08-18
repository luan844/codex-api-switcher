import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex max-w-full shrink-0 items-center border px-2 py-1 text-[11px] font-semibold tracking-[0.08em] uppercase whitespace-nowrap",
  {
    variants: {
      variant: {
        slate: "border-slate-200 bg-slate-100 text-slate-700",
        blue: "border-sky-200 bg-sky-50 text-sky-700",
        teal: "border-teal-200 bg-teal-50 text-teal-700",
        green: "border-emerald-200 bg-emerald-50 text-emerald-700",
        amber: "border-amber-200 bg-amber-50 text-amber-700",
        red: "border-rose-200 bg-rose-50 text-rose-700",
      },
    },
    defaultVariants: {
      variant: "slate",
    },
  },
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

export function Badge({ className, variant, ...props }: BadgeProps) {
  return <div className={cn(badgeVariants({ className, variant }))} {...props} />;
}
