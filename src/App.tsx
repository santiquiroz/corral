import { useCallback, useEffect, useState } from "react";
import ModelsTab from "./components/ModelsTab";
import SettingsTab from "./components/SettingsTab";
import StatusPill from "./components/StatusPill";
import StatusTab from "./components/StatusTab";
import { useSnapshot } from "./hooks/useSnapshot";
import { onNotice, pauseOllama, resumeOllama, takeNotices, unloadModel } from "./lib/api";

type Tab = "estado" | "modelos" | "ajustes";
const TABS: { id: Tab; label: string }[] = [
  { id: "estado", label: "Estado" },
  { id: "modelos", label: "Modelos" },
  { id: "ajustes", label: "Ajustes" },
];

async function unloadRunnerModels(model: string) {
  const results = await Promise.allSettled(model.split(" / ").map((name) => unloadModel(name)));
  const errors = results.filter((result) => result.status === "rejected");
  if (errors.length) throw new Error(errors.map((result) => String(result.reason)).join("; "));
}

export default function App() {
  const snapshot = useSnapshot();
  const [tab, setTab] = useState<Tab>("estado");
  const [busy, setBusy] = useState(false);
  const [notices, setNotices] = useState<string[]>([]);
  const addNotice = useCallback((message: string) => {
    setNotices((current) => current.includes(message) ? current : [...current, message]);
  }, []);
  const running = snapshot?.state.kind === "running";

  useEffect(() => {
    let active = true;
    const stop = onNotice((message) => { if (active) addNotice(message); });
    stop.then(() => takeNotices()).then((pending) => {
      if (active) pending.forEach(addNotice);
    }).catch((error) => { if (active) addNotice(String(error)); });
    return () => {
      active = false;
      stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [addNotice]);

  async function run(action: () => Promise<unknown>) {
    setBusy(true);
    try {
      await action();
    } catch (error) {
      addNotice(String(error));
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
      {notices.length > 0 && <div className="notice" role="status">
        {notices.map((message) => <p key={message}>{message}</p>)}
        <button onClick={() => setNotices([])}>Cerrar</button>
      </div>}
      <nav className="tabs" role="tablist">
        {TABS.map((t) => (
          <button key={t.id} role="tab" aria-selected={tab === t.id} onClick={() => setTab(t.id)}>{t.label}</button>
        ))}
      </nav>
      <main>
        {tab === "estado" && (snapshot ? <StatusTab snapshot={snapshot} onPause={() => run(pauseOllama)} onUnload={(model) => run(() => unloadRunnerModels(model))} busy={busy} /> : <p className="muted">Esperando la primera lectura…</p>)}
        {tab === "modelos" && <ModelsTab snapshot={snapshot} />}
        {tab === "ajustes" && <SettingsTab />}
      </main>
    </div>
  );
}
