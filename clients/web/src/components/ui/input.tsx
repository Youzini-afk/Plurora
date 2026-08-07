import { forwardRef, type ReactNode } from "react";
import { cn } from "@/lib/cn";

export interface FieldProps {
  label?: string;
  required?: boolean;
  helper?: string;
  error?: string;
  children: ReactNode;
  className?: string;
}

export function Field({ label, required, helper, error, children, className }: FieldProps) {
  return (
    <label className={cn("flex flex-col gap-2 text-charcoal-ink", className)}>
      {label ? (
        <span className="text-[12px] font-medium leading-none">
          {label}
          {required ? (
            <span className="ml-1 align-middle text-[14px] leading-none text-aged-brass">•</span>
          ) : null}
        </span>
      ) : null}
      {children}
      {error ? (
        <span className="text-[12px] text-deep-rust mt-1 leading-snug">{error}</span>
      ) : helper ? (
        <span className="text-[12px] text-steel-secondary leading-snug">{helper}</span>
      ) : null}
    </label>
  );
}

export const Input = forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>(
  ({ className, ...props }, ref) => (
    <input
      ref={ref}
      className={cn(
        "h-10 rounded-[10px] border border-whisper-border bg-transparent px-3 text-[13px] text-charcoal-ink placeholder:text-muted-tone outline-none transition focus-visible:border-aged-brass focus-visible:ring-2 focus-visible:ring-aged-brass/40",
        className,
      )}
      {...props}
    />
  ),
);
Input.displayName = "Input";

export interface InputGroupProps extends React.InputHTMLAttributes<HTMLInputElement> {
  leftIcon?: ReactNode;
  rightSlot?: ReactNode;
  inputClassName?: string;
}

export const InputGroup = forwardRef<HTMLInputElement, InputGroupProps>(
  ({ className, inputClassName, leftIcon, rightSlot, ...props }, ref) => (
    <div
      className={cn(
        "plurora-input-group flex h-10 items-center gap-2 rounded-[10px] border border-whisper-border bg-transparent px-3 transition focus-within:border-aged-brass focus-within:bg-pure-surface focus-within:shadow-[0_0_0_1px_var(--color-aged-brass)]",
        className,
      )}
    >
      {leftIcon ? <span className="text-steel-secondary shrink-0">{leftIcon}</span> : null}
      <input
        ref={ref}
        className={cn(
          "min-w-0 flex-1 bg-transparent text-[13px] text-charcoal-ink placeholder:text-muted-tone",
          inputClassName,
        )}
        {...props}
      />
      {rightSlot ? <span className="shrink-0 text-muted-tone">{rightSlot}</span> : null}
    </div>
  ),
);
InputGroup.displayName = "InputGroup";

export const Textarea = forwardRef<HTMLTextAreaElement, React.TextareaHTMLAttributes<HTMLTextAreaElement>>(
  ({ className, ...props }, ref) => (
    <textarea
      ref={ref}
      className={cn(
        "min-h-[80px] rounded-[10px] border border-whisper-border bg-transparent p-3 text-[13px] text-charcoal-ink placeholder:text-muted-tone outline-none transition focus-visible:border-aged-brass focus-visible:ring-2 focus-visible:ring-aged-brass/40",
        className,
      )}
      {...props}
    />
  ),
);
Textarea.displayName = "Textarea";

export interface CheckboxProps {
  checked?: boolean;
  disabled?: boolean;
  onCheckedChange?: (checked: boolean) => void;
  label: ReactNode;
  className?: string;
}

export function Checkbox({ checked, disabled, onCheckedChange, label, className }: CheckboxProps) {
  return (
    <label
      className={cn(
        "flex items-center gap-2 text-[12px] text-steel-secondary",
        disabled ? "cursor-not-allowed opacity-60" : "cursor-pointer",
        className,
      )}
    >
      <span
        className={cn(
          "relative inline-flex size-4 shrink-0 items-center justify-center rounded-[4px] border border-whisper-border-strong transition",
          checked && "border-aged-brass bg-aged-brass",
        )}
      >
        {checked ? (
          <svg
            viewBox="0 0 16 16"
            className="size-3"
            fill="none"
            stroke="var(--color-accent-foreground)"
            strokeWidth="2"
          >
            <path d="M3 8.5l3 3 7-7" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        ) : null}
      </span>
      <input
        type="checkbox"
        checked={!!checked}
        disabled={disabled}
        onChange={(event) => onCheckedChange?.(event.target.checked)}
        className="sr-only"
      />
      {label}
    </label>
  );
}
