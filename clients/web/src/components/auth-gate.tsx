import { useState, type FormEvent } from "react";
import { motion } from "motion/react";
import { Key, Eye, EyeSlash } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { InputGroup } from "@/components/ui/input";
import { useAuth } from "@/lib/auth-gate";
import { useT } from "@/lib/locale";
import { SPRING } from "@/lib/motion";
import { BrowserHostConnectionStore } from "@/client-core";

export function AuthGateScreen() {
  const { login, status, error } = useAuth();
  const t = useT();
  const [value, setValue] = useState("");
  const [show, setShow] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (!value.trim() || submitting) return;
    setSubmitting(true);
    try {
      await login(value.trim());
    } finally {
      setSubmitting(false);
    }
  };

  const isInvalid = status === "invalid";
  const showError = isInvalid && error;

  return (
    <div className="plurora-safe-overlay fixed inset-0 z-40 flex items-center justify-center bg-warm-bone/95 backdrop-blur-md dark:bg-deep-bark/95">
      <motion.div
        initial={{ opacity: 0, y: 24, scale: 0.97 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        transition={SPRING.modal}
        className="w-full max-w-[420px] rounded-2xl border border-whisper-border bg-pure-surface p-6 shadow-modal sm:p-8"
      >
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ ...SPRING.soft, delay: 0.06 }}
          className="flex flex-col items-center gap-1 text-center"
        >
          <span className="eyebrow mb-2">{t("authEyebrow")}</span>
          <h1 className="font-display text-[22px] font-bold tracking-[-0.015em] text-charcoal-ink">
            {t("authTitle")}
          </h1>
          <p className="mt-1 max-w-[32ch] text-[13px] leading-relaxed text-steel-secondary">
            {t("authBody")}
          </p>
        </motion.div>

        <motion.form
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ ...SPRING.soft, delay: 0.12 }}
          onSubmit={handleSubmit}
          className="mt-6 flex flex-col gap-3"
        >
          <InputGroup
            aria-label={t("authTokenLabel")}
            leftIcon={<Key size={16} />}
            rightSlot={
              <button
                type="button"
                onClick={() => setShow((s) => !s)}
                className="text-muted-tone hover:text-charcoal-ink"
                aria-label={show ? t("authHideToken") : t("authShowToken")}
              >
                {show ? <EyeSlash size={16} /> : <Eye size={16} />}
              </button>
            }
            type={show ? "text" : "password"}
            placeholder={t("authPlaceholder")}
            value={value}
            onChange={(e) => setValue(e.target.value)}
            spellCheck={false}
            autoComplete="off"
            autoFocus
          />

          {showError ? (
            <motion.p
              initial={{ opacity: 0, y: -4 }}
              animate={{ opacity: 1, y: 0 }}
              className="text-[12px] leading-snug text-deep-rust"
            >
              {error}
            </motion.p>
          ) : null}

          <Button
            tone="primary"
            size="lg"
            type="submit"
            disabled={submitting || !value.trim()}
            className="mt-1 w-full"
          >
            {submitting ? t("authCheckingButton") : t("authSubmitButton")}
          </Button>
        </motion.form>

        <motion.p
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ ...SPRING.soft, delay: 0.18 }}
          className="mt-4 text-center text-[11px] text-muted-tone"
        >
          {t("authStoredLocally")}
        </motion.p>
        <ManagedHostFallback />
      </motion.div>
    </div>
  );
}

export function AuthChecking() {
  const t = useT();
  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-warm-bone dark:bg-deep-bark">
      <div className="flex flex-col items-center gap-3">
        <span className="font-display text-[24px] font-bold tracking-[-0.015em] text-charcoal-ink">Plurora</span>
        <span className="pulse-dot text-[12px] text-muted-tone">{t("authCheckingAccess")}</span>
      </div>
    </div>
  );
}

export function HostUnavailable() {
  const { error, retry } = useAuth();
  const t = useT();
  const [retrying, setRetrying] = useState(false);

  const handleRetry = async () => {
    if (retrying) return;
    setRetrying(true);
    try {
      await retry();
    } finally {
      setRetrying(false);
    }
  };

  return (
    <div className="plurora-safe-overlay fixed inset-0 z-40 flex items-center justify-center bg-warm-bone/95 backdrop-blur-md dark:bg-deep-bark/95">
      <div className="w-full max-w-[460px] rounded-2xl border border-whisper-border bg-pure-surface p-6 text-center shadow-modal sm:p-8">
        <span className="eyebrow">{t("authEyebrow")}</span>
        <h1 className="mt-3 font-display text-[22px] font-bold tracking-[-0.015em] text-charcoal-ink">
          {t("homeErrorTitle")}
        </h1>
        <p className="mx-auto mt-2 max-w-[38ch] text-[13px] leading-relaxed text-steel-secondary">
          {t("homeErrorBody")}
        </p>
        {error ? <p className="mt-3 break-words font-mono text-[11px] text-muted-tone">{error}</p> : null}
        <Button tone="primary" size="lg" type="button" disabled={retrying} onClick={handleRetry} className="mt-6 w-full">
          {retrying ? t("authCheckingButton") : t("retry")}
        </Button>
        <ManagedHostFallback />
      </div>
    </div>
  );
}

function ManagedHostFallback() {
  const t = useT();
  const store = new BrowserHostConnectionStore();
  if (!store.active()) return null;
  return (
    <Button
      tone="secondary"
      type="button"
      className="mt-3 w-full"
      onClick={() => {
        store.select(undefined);
        window.location.reload();
      }}
    >
      {t("hostConnectionsReturnCurrent")}
    </Button>
  );
}
