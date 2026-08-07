import { type ReactNode, type ComponentProps } from "react";
import * as RadixDialog from "@radix-ui/react-dialog";
import { motion, AnimatePresence } from "motion/react";
import { cn } from "@/lib/cn";
import { SPRING, FADE } from "@/lib/motion";
import { X } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { useT } from "@/lib/locale";

export interface ModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  children: ReactNode;
  /** When set, renders a colored 4px left accent stripe (failure modal pattern). */
  accent?: "rust" | "brass" | null;
  /** Default 640px form width. Use 720 for plan/wizard layouts. */
  size?: "sm" | "md" | "lg";
  className?: string;
  contentLabel?: string;
}

const sizeWidths: Record<NonNullable<ModalProps["size"]>, string> = {
  sm: "w-[min(calc(100vw-24px),480px)]",
  md: "w-[min(calc(100vw-24px),640px)]",
  lg: "w-[min(calc(100vw-24px),760px)]",
};

export function Modal({ open, onOpenChange, children, accent = null, size = "md", className, contentLabel }: ModalProps) {
  const t = useT();
  return (
    <RadixDialog.Root open={open} onOpenChange={onOpenChange}>
      <AnimatePresence>
        {open ? (
          <RadixDialog.Portal forceMount>
            <RadixDialog.Overlay asChild>
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                transition={FADE.short}
                className="fixed inset-0 z-40 backdrop-blur-[8px]"
                style={{ background: "var(--color-overlay)" }}
              />
            </RadixDialog.Overlay>
            <RadixDialog.Content asChild aria-label={contentLabel}>
              <motion.div
                initial={{ opacity: 0, scale: 0.96 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.96 }}
                transition={SPRING.modal}
                className={cn(
                  "fixed left-1/2 top-1/2 z-50 -translate-x-1/2 -translate-y-1/2",
                  sizeWidths[size],
                  "plurora-safe-modal overflow-hidden rounded-[24px] bg-pure-surface shadow-modal border border-whisper-border",
                  accent === "rust" && "border-l-[4px] border-l-deep-rust",
                  accent === "brass" && "border-l-[4px] border-l-aged-brass",
                  className,
                )}
              >
                <div className="plurora-safe-modal relative overflow-y-auto p-6 sm:p-9">
                  <RadixDialog.Close asChild>
                    <Button
                      tone="icon"
                      size="icon-sm"
                      className="absolute right-4 top-4"
                      aria-label={t("uiModalClose")}
                    >
                      <X size={18} />
                    </Button>
                  </RadixDialog.Close>
                  {children}
                </div>
              </motion.div>
            </RadixDialog.Content>
          </RadixDialog.Portal>
        ) : null}
      </AnimatePresence>
    </RadixDialog.Root>
  );
}

export const ModalHeader = ({
  eyebrow,
  title,
  description,
  className,
}: {
  eyebrow?: string;
  title: string;
  description?: ReactNode;
  className?: string;
}) => (
  <header className={cn("mb-6 flex flex-col gap-2 pr-12", className)}>
    {eyebrow ? <p className="eyebrow">{eyebrow}</p> : null}
    <h2 className="font-display text-[24px] font-bold leading-tight tracking-[-0.02em] text-charcoal-ink">
      {title}
    </h2>
    {description ? (
      <p className="max-w-[60ch] text-[13px] leading-snug text-steel-secondary">{description}</p>
    ) : null}
  </header>
);

export const ModalFooter = ({ className, ...props }: ComponentProps<"footer">) => (
  <footer
    className={cn("mt-6 flex items-center gap-3 border-t border-whisper-border pt-4", className)}
    {...props}
  />
);

export const ModalTitle = RadixDialog.Title;
export const ModalDescription = RadixDialog.Description;
