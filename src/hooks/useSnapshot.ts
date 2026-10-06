import { useEffect, useState } from "react";
import { getSnapshot, onSnapshot } from "../lib/api";
import type { Snapshot } from "../lib/types";

export function useSnapshot(): Snapshot | null {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  useEffect(() => {
    let active = true;
    getSnapshot().then((s) => active && s && setSnapshot(s)).catch(() => {});
    const unlisten = onSnapshot((s) => active && setSnapshot(s));
    return () => {
      active = false;
      unlisten.then((stop) => stop());
    };
  }, []);
  return snapshot;
}
