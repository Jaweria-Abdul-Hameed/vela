import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Glass } from '../design';

/** One section of the Settings sheet. Tickets register their own (the Automation section is added by the consent work). */
export interface SettingsSection {
  id: string;
  title: string;
  /** Lower sorts first. */
  order: number;
  render: () => ReactNode;
}

export class SettingsSectionRegistry {
  private readonly sections = new Map<string, SettingsSection>();

  /** Adds or replaces a section by id. */
  register(section: SettingsSection): void {
    this.sections.set(section.id, section);
  }

  unregister(id: string): void {
    this.sections.delete(id);
  }

  list(): SettingsSection[] {
    return [...this.sections.values()].sort(
      (a, b) => a.order - b.order || a.title.localeCompare(b.title),
    );
  }
}

/** The sections named in SCREEN_INVENTORY.md; bodies are supplied by the tickets that own the settings. */
export const DEFAULT_SECTION_TITLES: ReadonlyArray<[id: string, title: string]> = [
  ['general', 'General'],
  ['appearance', 'Appearance'],
  ['motion', 'Motion and performance'],
  ['autonomy', 'Autonomy and policy'],
  ['git', 'Git'],
  ['adapters', 'Agent adapters'],
  ['tracker', 'Tracker'],
  ['notifications', 'Notifications'],
  ['diagnostics', 'Diagnostics'],
];

export function createSettingsRegistry(): SettingsSectionRegistry {
  const registry = new SettingsSectionRegistry();
  DEFAULT_SECTION_TITLES.forEach(([id, title], index) => {
    registry.register({
      id,
      title,
      order: (index + 1) * 10,
      render: () => <p>Nothing to configure here yet.</p>,
    });
  });
  return registry;
}

export interface SettingsSheetProps {
  registry: SettingsSectionRegistry;
  onClose: () => void;
  initialSection?: string;
}

/** A floating sheet, not a page: sections in a tab list, Escape closes, focus starts on the active tab. */
export function SettingsSheet({ registry, onClose, initialSection }: SettingsSheetProps) {
  const sections = registry.list();
  const [active, setActive] = useState(initialSection ?? sections[0]?.id ?? '');
  const tabRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    tabRef.current?.focus();
  }, []);
  const current = sections.find((s) => s.id === active) ?? sections[0];

  return (
    <Glass
      role="dialog"
      aria-modal="true"
      aria-label="Settings"
      onKeyDown={(e) => {
        if (e.key === 'Escape') onClose();
      }}
    >
      <div role="tablist" aria-label="Settings sections" aria-orientation="vertical">
        {sections.map((s) => (
          <button
            key={s.id}
            role="tab"
            type="button"
            id={`settings-tab-${s.id}`}
            aria-selected={s.id === current?.id}
            aria-controls={`settings-panel-${s.id}`}
            tabIndex={s.id === current?.id ? 0 : -1}
            ref={s.id === current?.id ? tabRef : undefined}
            onClick={() => setActive(s.id)}
          >
            {s.title}
          </button>
        ))}
      </div>
      {current ? (
        <section
          role="tabpanel"
          id={`settings-panel-${current.id}`}
          aria-labelledby={`settings-tab-${current.id}`}
        >
          <h2>{current.title}</h2>
          {current.render()}
        </section>
      ) : null}
      <button type="button" onClick={onClose}>
        Close
      </button>
    </Glass>
  );
}
