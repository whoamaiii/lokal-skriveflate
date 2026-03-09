interface DrawerToggleProps {
  side: "left" | "right";
  label: string;
  isOpen: boolean;
  controls: string;
  onToggle: () => void;
  onActivate?: () => void;
}

export function DrawerToggle({
  side,
  label,
  isOpen,
  controls,
  onToggle,
  onActivate,
}: DrawerToggleProps) {
  return (
    <button
      aria-controls={controls}
      aria-expanded={isOpen}
      aria-label={`${isOpen ? "Lukk" : "Åpne"} ${label.toLowerCase()}`}
      className={`drawer-toggle drawer-toggle-${side} ${isOpen ? "is-open" : ""}`}
      onClick={onToggle}
      onFocus={onActivate}
      onPointerDown={onActivate}
      type="button"
    >
      <span className="drawer-toggle-glow" aria-hidden="true" />
      <span className="drawer-toggle-clip" aria-hidden="true">
        <span className="drawer-toggle-rotor" />
      </span>
      <span className="drawer-toggle-plate" aria-hidden="true" />
      <span className="drawer-toggle-content">
        <span className="drawer-toggle-kicker">{isOpen ? "Lukk" : "Åpne"}</span>
        <span className="drawer-toggle-label">{label}</span>
      </span>
    </button>
  );
}
