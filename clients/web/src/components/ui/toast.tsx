import { createContext, useCallback, useContext, useMemo, useRef, useState, type ReactNode } from "react";
import { motion, AnimatePresence } from "motion/react";
import { CheckCircle, Cloud, Info, Warning, X, XCircle } from "@/components/icons";
import { SPRING } from "@/lib/motion";
import { cn } from "@/lib/cn";
import { useT } from "@/lib/locale";

export type ToastVariant = "info" | "success" | "warning" | "error" | "progress";

export interface ToastInput {
  id?: string;
  variant?: ToastVariant;
  title: string;
  body?: string;
  duration?: number; // ms; null/0 = no auto-dismiss
  action?: { label: string; onClick: () => void };
  /** For progress variant: [0, 1] fraction. */
  progress?: number;
}

interface ToastInternal extends ToastInput {
  id: string;
  variant: ToastVariant;
}

interface ToastContextValue {
  push: (toast: ToastInput) => string;
  update: (id: string, patch: Partial<ToastInput>) => void;
  dismiss: (id: string) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

const variantIcon = {
  info: Info,
  success: CheckCircle,
  warning: Warning,
  error: XCircle,
  progress: Cloud,
};

const variantStripe: Record<ToastVariant, string> = {
  info: "bg-aged-brass",
  success: "bg-aged-brass",
  warning: "bg-aged-brass-deep",
  error: "bg-deep-rust",
  progress: "bg-aged-brass",
};

const variantTextTone: Record<ToastVariant, string> = {
  info: "text-aged-brass-deep",
  success: "text-aged-brass-deep",
  warning: "text-aged-brass-deep",
  error: "text-deep-rust",
  progress: "text-aged-brass-deep",
};

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastInternal[]>([]);
  const timers = useRef(new Map<string, number>());

  const dismiss = useCallback((id: string) => {
    setToasts((current) => current.filter((toast) => toast.id !== id));
    const timer = timers.current.get(id);
    if (timer) {
      window.clearTimeout(timer);
      timers.current.delete(id);
    }
  }, []);

  const push = useCallback(
    (input: ToastInput) => {
      const id = input.id ?? crypto.randomUUID();
      const variant = input.variant ?? "info";
      const toast: ToastInternal = { ...input, id, variant };
      // Keep at most 3 toasts visible per spec; drop oldest.
      setToasts((current) => [...current.slice(-2), toast]);
      const duration = input.duration ?? (variant === "error" || variant === "progress" ? 0 : 4400);
      if (duration > 0) {
        const timer = window.setTimeout(() => dismiss(id), duration);
        timers.current.set(id, timer);
      }
      return id;
    },
    [dismiss],
  );

  const update = useCallback((id: string, patch: Partial<ToastInput>) => {
    setToasts((current) =>
      current.map((toast) => (toast.id === id ? { ...toast, ...patch } : toast)),
    );
  }, []);

  const value = useMemo<ToastContextValue>(() => ({ push, update, dismiss }), [push, update, dismiss]);

  return (
    <ToastContext.Provider value={value}>
      {children}
      <ToastViewport toasts={toasts} dismiss={dismiss} />
    </ToastContext.Provider>
  );
}

function ToastViewport({
  toasts,
  dismiss,
}: {
  toasts: ToastInternal[];
  dismiss: (id: string) => void;
}) {
  const t = useT();
  return (
    <ol
      role="status"
      aria-live="polite"
      className="plurora-safe-toast pointer-events-none fixed z-50 flex flex-col gap-2"
    >
      <AnimatePresence initial={false}>
        {toasts.map((toast) => {
          const Icon = variantIcon[toast.variant];
          return (
            <motion.li
              key={toast.id}
              layout
              initial={{ opacity: 0, y: 16, scale: 0.97 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: 12, scale: 0.97 }}
              transition={SPRING.snap}
              className="pointer-events-auto relative overflow-hidden rounded-[16px] border border-whisper-border bg-pure-surface shadow-toast"
            >
              <span className={cn("absolute inset-y-0 left-0 w-1", variantStripe[toast.variant])} />
              <div className="flex gap-3 p-4 pl-5">
                <span className={cn("mt-0.5 shrink-0", variantTextTone[toast.variant])}>
                  <Icon size={18} />
                </span>
                <div className="flex flex-1 flex-col gap-1 text-charcoal-ink">
                  <div className="flex items-start justify-between gap-2">
                    <p className="text-[13px] font-medium leading-snug">{toast.title}</p>
                    <button
                      type="button"
                      aria-label={t("uiToastDismiss")}
                      onClick={() => dismiss(toast.id)}
                      className="-mr-1 -mt-1 shrink-0 rounded p-1 text-muted-tone hover:text-charcoal-ink"
                    >
                      <X size={14} />
                    </button>
                  </div>
                  {toast.body ? (
                    <p className="text-[12px] leading-snug text-steel-secondary">{toast.body}</p>
                  ) : null}
                  {toast.variant === "progress" && typeof toast.progress === "number" ? (
                    <div className="mt-1 h-1 w-full overflow-hidden rounded-full bg-whisper-border-strong/40">
                      <div
                        className="h-full bg-aged-brass transition-[width] duration-300"
                        style={{ width: `${Math.max(0, Math.min(1, toast.progress)) * 100}%` }}
                      />
                    </div>
                  ) : null}
                  {toast.action ? (
                    <button
                      type="button"
                      onClick={toast.action.onClick}
                      className="mt-1 self-start text-[12px] font-medium text-charcoal-ink underline underline-offset-4 hover:decoration-aged-brass"
                    >
                      {toast.action.label} →
                    </button>
                  ) : null}
                </div>
              </div>
            </motion.li>
          );
        })}
      </AnimatePresence>
    </ol>
  );
}

export function useToast(): ToastContextValue {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error("useToast must be used inside <ToastProvider>");
  return ctx;
}
