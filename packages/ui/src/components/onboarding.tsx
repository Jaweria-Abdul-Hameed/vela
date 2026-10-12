import { useState } from 'react';
import { Glass } from '../design';

export interface OnboardingChoices {
  background: boolean;
  autostart: boolean;
  uiAutomation: boolean;
  recoveryContinuation: 'ask' | 'auto_safe';
}

/** Every choice starts on the conservative answer. */
export const CONSERVATIVE_CHOICES: OnboardingChoices = {
  background: false,
  autostart: false,
  uiAutomation: false,
  recoveryContinuation: 'ask',
};

export interface ConsentCopy {
  key: 'background' | 'autostart' | 'ui_automation';
  /** The version recorded with the decision. */
  version: string;
  text: string;
}

export interface OnboardingSheetProps {
  consents: ConsentCopy[];
  /** Called with the choices; the caller records each with the version of the text shown. */
  onComplete: (choices: OnboardingChoices, shownVersions: Record<string, string>) => void;
  /** Revisits skip the welcome and start from the saved choices. */
  initial?: OnboardingChoices;
}

const FIELD: Record<ConsentCopy['key'], keyof OnboardingChoices> = {
  background: 'background',
  autostart: 'autostart',
  ui_automation: 'uiAutomation',
};

export function OnboardingSheet({ consents, onComplete, initial }: OnboardingSheetProps) {
  const [choices, setChoices] = useState<OnboardingChoices>(initial ?? CONSERVATIVE_CHOICES);
  return (
    <Glass role="dialog" aria-modal="true" aria-labelledby="onboarding-title">
      <h2 id="onboarding-title">Welcome to Vela</h2>
      <p>
        Everything below is off unless you turn it on. You can change any of it later in Settings.
      </p>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          onComplete(choices, Object.fromEntries(consents.map((c) => [c.key, c.version])));
        }}
      >
        {consents.map((c) => {
          const field = FIELD[c.key];
          return (
            <label key={c.key}>
              <input
                type="checkbox"
                checked={choices[field] as boolean}
                onChange={(e) => setChoices({ ...choices, [field]: e.target.checked })}
              />{' '}
              {c.text}
            </label>
          );
        })}
        <label>
          When Vela finds an interrupted run:{' '}
          <select
            value={choices.recoveryContinuation}
            onChange={(e) =>
              setChoices({
                ...choices,
                recoveryContinuation: e.target.value as OnboardingChoices['recoveryContinuation'],
              })
            }
          >
            <option value="ask">Ask me</option>
            <option value="auto_safe">Continue when it is safe</option>
          </select>
        </label>
        <button type="submit">Continue</button>
      </form>
    </Glass>
  );
}
