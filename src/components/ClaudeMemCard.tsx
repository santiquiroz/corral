import { useEffect, useState } from "react";
import { claudeMemStatus } from "../lib/api";
import type { Check, ClaudeMemStatus } from "../lib/types";

const CHECK_ICONS: Record<Check["level"], string> = { ok: "✓", warn: "!", fail: "✕" };

function useClaudeMemStatus() {
  const [status, setStatus] = useState<ClaudeMemStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    let pending = false;
    async function refresh() {
      if (pending) return;
      pending = true;
      try {
        const next = await claudeMemStatus();
        if (active) {
          setStatus(next);
          setError(null);
        }
      } catch (error) {
        if (active) setError(String(error));
      } finally {
        pending = false;
      }
    }
    void refresh();
    const timer = setInterval(() => { void refresh(); }, 10000);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, []);

  return { status, error };
}

export default function ClaudeMemCard() {
  const { status, error } = useClaudeMemStatus();
  const hasFailure = status?.checks.some((check) => check.level === "fail");
  return <>
    {error && <p className="notice" role="status">{error}</p>}
    {status && <section className="claude-mem-card" aria-label="claude-mem" style={{ borderColor: hasFailure ? "var(--bad)" : undefined }}>
      <h2>claude-mem</h2>
      <dl>
        <dt>Proveedor</dt><dd>{status.provider}</dd>
        <dt>Endpoint</dt><dd className="mono">{status.base_url}</dd>
        <dt>Modelo</dt><dd className="mono">{status.model}</dd>
      </dl>
      <p>Cola: {status.queue_depth ?? "—"}</p>
      <ul>{status.checks.map((check) => <li key={check.id}>{CHECK_ICONS[check.level]} {check.message}</li>)}</ul>
    </section>}
  </>;
}
