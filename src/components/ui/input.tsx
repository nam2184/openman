import * as React from "react";
import { cn } from "../../lib/utils";

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {}

export const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ className, type, ...props }, ref) => (
    <input
      type={type}
      className={cn(
        "flex h-9 w-full rounded-md border border-[#45475a] bg-[#1e1e2e] px-3 py-1 text-sm text-[#cdd6f4] shadow-sm transition-colors placeholder:text-[#6c7086] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#89b4fa] disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      ref={ref}
      {...props}
    />
  ),
);

Input.displayName = "Input";
