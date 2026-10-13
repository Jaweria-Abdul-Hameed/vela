import { useState } from 'react';
import { GRAPHICS_MODES, QUALITY, type GraphicsMode } from '../canvas/quality';

export interface HomeProject {
  id: string;
  name: string;
  repoPath: string;
}

export interface ProjectHomeProps {
  projects: HomeProject[];
  selectedId: string | null;
  /** An import is in flight. */
  busy?: boolean;
  /** A folder is being dragged over the window. */
  dropActive?: boolean;
  onImport: (folder: string) => void;
  onOpen: (id: string) => void;
  onRemove: (id: string) => void;
}

/** The first thing you see: an open invitation to drop a repository folder, and your recent projects as quiet anchors. */
export function ProjectHome({
  projects,
  selectedId,
  busy = false,
  dropActive = false,
  onImport,
  onOpen,
  onRemove,
}: ProjectHomeProps) {
  const [folder, setFolder] = useState('');
  return (
    <section aria-label="Projects" data-drop-active={dropActive}>
      <h2>{projects.length === 0 ? 'Start with a repository' : 'Projects'}</h2>
      <p>
        {dropActive
          ? 'Release to import this folder.'
          : 'Drop a repository folder anywhere in this window, or type its path.'}
      </p>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          const path = folder.trim();
          if (path === '' || busy) return;
          onImport(path);
          setFolder('');
        }}
      >
        <label>
          Repository folder{' '}
          <input
            value={folder}
            onChange={(e) => setFolder(e.target.value)}
            placeholder="C:\Users\you\Projects\app"
            disabled={busy}
          />
        </label>{' '}
        <button type="submit" disabled={folder.trim() === '' || busy}>
          {busy ? 'Importing\u2026' : 'Import'}
        </button>
      </form>
      {projects.length === 0 ? <p>No projects yet. Import a repository to begin.</p> : null}
      <ul aria-label="Recent projects">
        {projects.map((p) => (
          <li key={p.id}>
            <button type="button" aria-pressed={p.id === selectedId} onClick={() => onOpen(p.id)}>
              {p.name}
            </button>{' '}
            <code>{p.repoPath}</code>{' '}
            <button type="button" aria-label={`Remove ${p.name}`} onClick={() => onRemove(p.id)}>
              Remove from Vela
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}

export interface GraphicsControlProps {
  mode: GraphicsMode;
  onChange: (mode: GraphicsMode) => void;
  /** A suggestion from the adaptive logic; the user's setting stays visible and unchanged until they accept. */
  suggestion?: { mode: GraphicsMode; reason: string } | null;
  onDismissSuggestion?: () => void;
}

const LABEL: Record<GraphicsMode, string> = {
  full: 'Full',
  balanced: 'Balanced',
  efficiency: 'Efficiency',
};

/** The graphics mode, with what each one costs, and a suggestion that never changes the setting by itself. */
export function GraphicsControl({
  mode,
  onChange,
  suggestion,
  onDismissSuggestion,
}: GraphicsControlProps) {
  return (
    <section aria-label="Graphics settings">
      <label>
        Graphics{' '}
        <select value={mode} onChange={(e) => onChange(e.target.value as GraphicsMode)}>
          {GRAPHICS_MODES.map((m) => (
            <option key={m} value={m}>
              {LABEL[m]}
            </option>
          ))}
        </select>
      </label>
      <p>{QUALITY[mode].description}</p>
      {suggestion ? (
        <p role="status">
          {suggestion.reason} Switch to {LABEL[suggestion.mode]}?{' '}
          <button type="button" onClick={() => onChange(suggestion.mode)}>
            Switch
          </button>{' '}
          <button type="button" onClick={onDismissSuggestion}>
            Keep {LABEL[mode]}
          </button>
        </p>
      ) : null}
    </section>
  );
}
