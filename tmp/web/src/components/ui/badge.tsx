import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "@/components/ui/cn"

const badgeVariants = cva(
  "inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium transition-colors",
  {
    variants: {
      variant: {
        default: "bg-[var(--accent)]/15 text-[var(--accent)]",
        secondary: "bg-[var(--bg-hover)] text-[var(--text)]",
        success: "bg-[var(--ok)]/15 text-[var(--ok)]",
        warning: "bg-[var(--warn)]/15 text-[var(--warn)]",
        destructive: "bg-[var(--danger)]/15 text-[var(--danger)]",
        outline: "border border-[var(--border)] text-[var(--muted)]",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
)

export interface BadgeProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return <div className={cn(badgeVariants({ variant }), className)} {...props} />
}

export { Badge, badgeVariants }
