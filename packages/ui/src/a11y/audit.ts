/** Accessibility rules as code (U13): what must hold everywhere, checked by tests so a regression is a failing build rather than a
 *  design review comment. */

import { NODE_VISUALS, type NodeState } from '../canvas/graph/visuals';

/** Every keyboard shortcut Vela offers, in one place so the help dialog cannot drift from the code. */
export interface Shortcut {
  keys: string;
  does: string;
  scope: 'anywhere' | 'graph' | 'inspector' | 'run';
}

export const SHORTCUTS: readonly Shortcut[] = [
  { keys: '?', does: 'Show this list of shortcuts', scope: 'anywhere' },
  { keys: 'Ctrl+K', does: 'Open the command palette', scope: 'anywhere' },
  { keys: 'Tab / Shift+Tab', does: 'Move between controls', scope: 'anywhere' },
  { keys: 'Arrow keys', does: 'Move between tickets in the list', scope: 'graph' },
  { keys: 'Right / Left', does: 'Open or close a ticket group', scope: 'graph' },
  { keys: 'Home / End', does: 'Go to the first or last ticket', scope: 'graph' },
  { keys: 'Enter', does: 'Open the selected ticket', scope: 'graph' },
  { keys: '+ / -', does: 'Zoom the graph', scope: 'graph' },
  { keys: '0', does: 'Reset the graph view', scope: 'graph' },
  { keys: 'Escape', does: 'Close the inspector or a dialog', scope: 'inspector' },
  { keys: 'Ctrl+Shift+.', does: 'Stop all workers (asks first)', scope: 'run' },
];

/** The glyph a state is drawn with, named in words, so "which shape" is something a test and a screen reader can both state. */
export function describeState(state: NodeState): string {
  const v = NODE_VISUALS[state];
  return `${v.label} (${v.shape})`;
}

/** Whether any two states differ only in colour. Returns the offending pairs (empty means status is never hue-only). */
export function hueOnlyPairs(): [NodeState, NodeState][] {
  const states = Object.keys(NODE_VISUALS) as NodeState[];
  const bad: [NodeState, NodeState][] = [];
  for (let i = 0; i < states.length; i++) {
    for (let j = i + 1; j < states.length; j++) {
      const a = NODE_VISUALS[states[i]!];
      const b = NODE_VISUALS[states[j]!];
      // Distinguishable without colour: the written label always differs, which is what a screen reader and a user who cannot
      // tell hues apart both read.
      if (a.label === b.label && a.shape === b.shape) bad.push([states[i]!, states[j]!]);
    }
  }
  return bad;
}

/** WCAG 2.3.1: nothing may flash more than three times in any one second. `periodMs` is the full on-off cycle. */
export function flashesPerSecond(periodMs: number): number {
  return periodMs > 0 ? 1000 / periodMs : 0;
}

/** The breathing of a running node: one slow cycle, far under the flash limit. */
export const PULSE_PERIOD_MS = 2400;

export const MAX_FLASHES_PER_SECOND = 3;

/** Minimum scalable text size in CSS pixels at 100% zoom; sizes are rem-based so browser and OS text scaling apply. */
export const MIN_TEXT_PX = 12;
