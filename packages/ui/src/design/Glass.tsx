import type { HTMLAttributes } from 'react';

/** The only place the glass treatment is applied: floating surfaces (sheets, run bar, palette), never page chrome. */
export function Glass({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return <div {...rest} className={['vela-glass', className].filter(Boolean).join(' ')} />;
}
