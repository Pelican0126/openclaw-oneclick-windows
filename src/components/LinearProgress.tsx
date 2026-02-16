interface LinearProgressProps {
  label: string;
  value?: number | null;
  hint?: string;
  indeterminate?: boolean;
  active?: boolean;
  className?: string;
}

// Lightweight progress bar (determinate/indeterminate) used across the installer to avoid
// repeating ad-hoc markup and to keep progress visuals consistent.
export function LinearProgress({
  label,
  value,
  hint,
  indeterminate = false,
  active = false,
  className = ""
}: LinearProgressProps) {
  const isIndeterminate = indeterminate || value == null;
  const safeValue = Math.max(0, Math.min(100, Math.round(value ?? 0)));
  const panelClass = className ? `progress-panel ${className}` : "progress-panel";

  return (
    <div className={panelClass}>
      <div className="progress-meta">
        <span>{label}</span>
        <strong>{isIndeterminate ? "..." : `${safeValue}%`}</strong>
      </div>
      <div className="progress-track" aria-label={label}>
        {isIndeterminate ? (
          <div className="progress-indeterminate" />
        ) : (
          <div className={active ? "progress-fill running" : "progress-fill"} style={{ width: `${safeValue}%` }} />
        )}
      </div>
      {hint ? <div className="muted-inline">{hint}</div> : null}
    </div>
  );
}
