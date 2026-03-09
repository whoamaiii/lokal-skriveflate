import { House, Moon, Search, Sun, UserRound } from "lucide-react";
import { useEffect, useEffectEvent, useRef, useState } from "react";

export type NavSection = "home" | "search" | "user";
export type ThemeMode = "dark" | "light";

interface FloatingNavProps {
  activeNav: NavSection;
  isThemeAnimating: boolean;
  onSelectNav: (section: NavSection) => void;
  onToggleTheme: () => void;
  themeMode: ThemeMode;
}

interface IndicatorFrame {
  ready: boolean;
  width: number;
  x: number;
}

const navItems = [
  { icon: House, id: "home" as const, label: "Hjem" },
  { icon: Search, id: "search" as const, label: "Dokument" },
  { icon: UserRound, id: "user" as const, label: "Assistent" },
];

export function FloatingNav({
  activeNav,
  isThemeAnimating,
  onSelectNav,
  onToggleTheme,
  themeMode,
}: FloatingNavProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const navButtonRefs = useRef<Record<NavSection, HTMLButtonElement | null>>({
    home: null,
    search: null,
    user: null,
  });
  const [indicatorFrame, setIndicatorFrame] = useState<IndicatorFrame>({
    ready: false,
    width: 0,
    x: 0,
  });

  const syncIndicator = useEffectEvent(() => {
    const container = containerRef.current;
    const button = navButtonRefs.current[activeNav];

    if (!container || !button) {
      return;
    }

    const containerRect = container.getBoundingClientRect();
    const buttonRect = button.getBoundingClientRect();

    setIndicatorFrame({
      ready: true,
      width: buttonRect.width,
      x: buttonRect.left - containerRect.left,
    });
  });

  useEffect(() => {
    syncIndicator();

    const resizeObserver = new ResizeObserver(() => {
      syncIndicator();
    });

    if (containerRef.current) {
      resizeObserver.observe(containerRef.current);
    }

    for (const button of Object.values(navButtonRefs.current)) {
      if (button) {
        resizeObserver.observe(button);
      }
    }

    window.addEventListener("resize", syncIndicator);

    return () => {
      resizeObserver.disconnect();
      window.removeEventListener("resize", syncIndicator);
    };
  }, [activeNav, syncIndicator]);

  return (
    <div className="floating-nav-shell">
      <div
        className={`floating-nav ${indicatorFrame.ready ? "is-ready" : ""}`}
        data-theme-mode={themeMode}
        ref={containerRef}
      >
        <div className="floating-nav-noise" />
        <div
          className="floating-nav-indicator"
          style={{
            opacity: indicatorFrame.ready ? 1 : 0,
            transform: `translate3d(${indicatorFrame.x}px, 0, 0)`,
            width: `${indicatorFrame.width}px`,
          }}
        >
          <div className="floating-nav-indicator-glow" />
          <div className="floating-nav-indicator-clip">
            <div className="floating-nav-indicator-rotor" />
          </div>
          <div className="floating-nav-indicator-plate" />
        </div>

        <nav className="floating-nav-buttons" aria-label="Navigasjon i arbeidsflaten">
          {navItems.map(({ icon: Icon, id, label }, index) => (
            <div className="floating-nav-item" key={id}>
              <button
                aria-pressed={activeNav === id}
                className="floating-nav-button"
                onClick={() => onSelectNav(id)}
                ref={(node) => {
                  navButtonRefs.current[id] = node;
                }}
                type="button"
              >
                <Icon size={18} strokeWidth={1.8} />
                <span>{label}</span>
              </button>
              {index < navItems.length - 1 ? <span className="floating-nav-divider" aria-hidden="true" /> : null}
            </div>
          ))}
        </nav>

        <button
          aria-label={themeMode === "dark" ? "Bytt til lyst tema" : "Bytt til mørkt tema"}
          className={`floating-nav-button floating-nav-theme-toggle ${
            isThemeAnimating ? "is-bouncing" : ""
          }`}
          data-theme-mode={themeMode}
          onClick={onToggleTheme}
          type="button"
        >
          <span className="floating-nav-theme-icons" aria-hidden="true">
            <Sun className="floating-nav-theme-icon floating-nav-theme-icon-sun" size={17} strokeWidth={1.8} />
            <Moon className="floating-nav-theme-icon floating-nav-theme-icon-moon" size={17} strokeWidth={1.8} />
          </span>
        </button>
      </div>
    </div>
  );
}
