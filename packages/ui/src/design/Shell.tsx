import type { ReactNode } from 'react';

/** The base shell: quiet branding, one main region, no permanent sidebar. */
export function Shell({ children }: { children: ReactNode }) {
  return (
    <div className="vela-shell">
      <header className="vela-shell__brand">
        <h1>Vela</h1>
      </header>
      <main className="vela-shell__main" id="main">
        {children}
      </main>
    </div>
  );
}
