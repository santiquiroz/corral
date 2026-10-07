import { useState } from "react";
import ModelsTab from "./components/ModelsTab";
import SettingsTab from "./components/SettingsTab";
import StatusPill from "./components/StatusPill";
import StatusTab from "./components/StatusTab";
import { useSnapshot } from "./hooks/useSnapshot";
import { pauseOllama, resumeOllama } from "./lib/api";

type Tab = "estado" | "modelos" | "ajustes";
const TABS: { id: Tab; label: string }[] = [
  { id: "estado", label: "Estado" },
  { id: "modelos", label: "Modelos" },
  { id: "ajustes", label: "Ajustes" },
];

export default function App() {
  const snapshot = useSnapshot();
  const [tab, setTab] = useState<Tab>("estado");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const running = snapshot?.state.kind === "running";

  async function run(action: () => Promise<unknown>) {
    setBusy(true);
    setNotice(null);
    try {
      await action();
    } catch (error) {
      setNotice(String(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="app">
      <header className="top">
        <h1>Corral</h1>
        <StatusPill snapshot={snapshot} />
        <button className="primary" disabled={busy || !snapshot} onClick={() => run(running ? pauseOllama : resumeOllama)}>
          {running ? "Pausar Ollama" : "Reanudar Ollama"}
        </button>
      </header>
      {notice && <p className="notice" role="status">{notice}</p>}
      <nav className="tabs" role="tablist">
        {TABS.map((t) => (
          <button key={t.id} role="tab" aria-selected={tab === t.id} onClick={() => setTab(t.id)}>{t.label}</button>
        ))}
      </nav>
      <main>
        {tab === "estado" && (snapshot ? <StatusTab snapshot={snapshot} onPause={() => run(pauseOllama)} busy={busy} /> : <p className="muted">Esperando la primera lectura…</p>)}
        {tab === "modelos" && <ModelsTab snapshot={snapshot} />}
        {tab === "ajustes" && <SettingsTab />}
      </main>
    </div>
  );
}
