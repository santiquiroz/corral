export function formatMb(mb: number | null): string {
  if (mb === null) return "sin datos";
  return mb < 1024 ? `${Math.round(mb)} MB` : `${(mb / 1024).toFixed(1)} GB`;
}

export function formatPct(part: number, total: number): string {
  return total > 0 ? `${Math.round((part / total) * 100)} %` : "0 %";
}
