import { useEffect, useRef } from 'react';
import { Glass } from '../design';

/** Everything the core records about a human-gated stop (the intervention record). */
export interface InterventionView {
  id: string;
  /** The intervention kind, in the specification's wire form (for example `APPROVAL_ASK`). */
  kind: string;
  summary: string;
  evidence: string[];
  preservedWork: string;
  lastSafeCommit?: string;
  consequences: string;
  /** Scope text of the rule a user could create, for approval kinds. */
  ruleScope?: string;
}

export type InterventionAction =
  | 'resolve'
  | 'allow_once'
  | 'keep_refusing'
  | 'create_rule'
  | 'recheck_access'
  | 'view_evidence'
  | 'cancel';

interface Variant {
  title: string;
  /** Plain statement of what happened. */
  blurb: string;
  actions: InterventionAction[];
}

const APPROVAL: InterventionAction[] = ['allow_once', 'create_rule', 'keep_refusing', 'cancel'];
const GENERIC: InterventionAction[] = ['resolve', 'view_evidence', 'cancel'];

/** Copy and actions per kind (COPY_AND_TONE.md: plain, specific, no blame). */
export const VARIANTS: Record<string, Variant> = {
  AMBIGUITY: {
    title: 'The ticket needs a decision',
    blurb: 'The worker found the ticket unclear or contradictory and stopped instead of guessing.',
    actions: GENERIC,
  },
  REVIEW_STALLED: {
    title: 'The review is stuck',
    blurb: 'Reviews are not converging, so Vela stopped rather than loop.',
    actions: GENERIC,
  },
  APPROVAL_ASK: {
    title: 'Allow this operation?',
    blurb: 'The policy does not allow this on its own, so it is asking you.',
    actions: APPROVAL,
  },
  APPROVAL_STALLED: {
    title: 'A worker seems to be waiting',
    blurb:
      'The same request keeps coming back, or the worker went quiet. It may be waiting on an approval nobody answered.',
    actions: GENERIC,
  },
  APPROVAL_UNDELIVERABLE: {
    title: 'Vela cannot deliver the approval',
    blurb:
      'The policy allows this, but there is no safe, permitted way to pass the approval to Antigravity.',
    actions: GENERIC,
  },
  POLICY_DENIED: {
    title: 'The policy keeps refusing this',
    blurb:
      'The worker keeps asking for something the policy refuses, or needs it to finish the ticket.',
    actions: ['keep_refusing', 'view_evidence', 'cancel'],
  },
  POLICY_VIOLATION: {
    title: 'A safety control fired',
    blurb: 'Something happened that the policy forbids. Review the evidence before continuing.',
    actions: GENERIC,
  },
  MERGE_CONFLICT: {
    title: 'This change conflicts with the others',
    blurb: 'Merging would conflict. Nothing was changed on the integration branch.',
    actions: GENERIC,
  },
  INTEGRATION_REGRESSION: {
    title: 'Integration keeps failing',
    blurb: 'After merging, the checks keep failing for this ticket. Its work is preserved.',
    actions: GENERIC,
  },
  AUTH_REQUIRED: {
    title: 'Sign in to continue',
    blurb: 'Antigravity needs you to sign in. Vela never handles your credentials.',
    actions: ['recheck_access', 'view_evidence', 'cancel'],
  },
  PROVIDER_POLICY_BLOCK: {
    title: 'The provider restricted access',
    blurb:
      'The provider has restricted this account or service. Vela will not retry and will not work around it. Your work is preserved.',
    actions: ['recheck_access', 'view_evidence', 'cancel'],
  },
  EXTERNAL_DIVERGENCE: {
    title: 'Something changed outside Vela',
    blurb: 'Branches, worktrees or issues were changed behind Vela’s back.',
    actions: GENERIC,
  },
  GRAPH_INVALID: {
    title: 'The issues contain a cycle',
    blurb: 'Some issues depend on each other in a loop, so no order exists. Nothing was started.',
    actions: GENERIC,
  },
  INTEGRATION_UNKNOWN: {
    title: 'Integration is unverified',
    blurb: 'Vela could not confirm the integration branch is healthy.',
    actions: GENERIC,
  },
  INTEGRATION_UNHEALTHY: {
    title: 'Integration is not trustworthy',
    blurb:
      'A failed merge could not be discarded cleanly. Nothing new starts until it is restored.',
    actions: GENERIC,
  },
  OTHER: {
    title: 'Vela needs a decision',
    blurb: 'Something needs your attention before work can continue.',
    actions: GENERIC,
  },
};

export const ACTION_LABEL: Record<InterventionAction, string> = {
  resolve: 'Resolve and continue',
  allow_once: 'Allow once',
  keep_refusing: 'Keep refusing',
  create_rule: 'Always allow this…',
  recheck_access: 'Check access once',
  view_evidence: 'View evidence',
  cancel: 'Cancel',
};

function variantOf(kind: string): Variant {
  return VARIANTS[kind] ?? VARIANTS['OTHER']!;
}

/** The actions for `kind`. A rule can only be offered when the caller supplied its scope. */
export function actionsFor(kind: string, hasRuleScope: boolean): InterventionAction[] {
  return variantOf(kind).actions.filter((a) => a !== 'create_rule' || hasRuleScope);
}

export interface InterventionSheetProps {
  intervention: InterventionView;
  onAction: (action: InterventionAction) => void;
}

/** One human-gated stop as a card: what is blocked, what is safe, what the evidence says, and what each action leads to. */
export function InterventionSheet({ intervention: i, onAction }: InterventionSheetProps) {
  const variant = variantOf(i.kind);
  const first = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    first.current?.focus();
  }, []);
  const actions = actionsFor(i.kind, i.ruleScope !== undefined);
  return (
    <Glass role="dialog" aria-modal="true" aria-labelledby={`iv-${i.id}`}>
      <h2 id={`iv-${i.id}`}>{variant.title}</h2>
      <p>{variant.blurb}</p>
      <p>{i.summary}</p>
      <section aria-label="Preserved work">
        <h3>Your work is safe</h3>
        <p>{i.preservedWork}</p>
        {i.lastSafeCommit ? (
          <p>
            Last safe commit: <code>{i.lastSafeCommit}</code>
          </p>
        ) : null}
      </section>
      {i.evidence.length > 0 ? (
        <section aria-label="Evidence">
          <h3>Evidence</h3>
          <ul>
            {i.evidence.map((e, n) => (
              <li key={n}>{e}</li>
            ))}
          </ul>
        </section>
      ) : null}
      {i.ruleScope ? <p aria-label="Rule scope">{i.ruleScope}</p> : null}
      <p>{i.consequences}</p>
      <div role="group" aria-label="Actions">
        {actions.map((a, n) => (
          <button
            key={a}
            type="button"
            ref={n === 0 ? first : undefined}
            onClick={() => onAction(a)}
          >
            {ACTION_LABEL[a]}
          </button>
        ))}
      </div>
    </Glass>
  );
}
