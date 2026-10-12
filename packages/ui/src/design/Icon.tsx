import type { SVGProps } from 'react';

/** One geometric family: 24px grid, 1.5 stroke, round caps. */
const PATHS = {
  plus: 'M12 5v14M5 12h14',
  close: 'M6 6l12 12M18 6L6 18',
  check: 'M5 12.5l4.5 4.5L19 7.5',
  chevron: 'M9 6l6 6-6 6',
  stop: 'M7 7h10v10H7z',
  play: 'M8 6l10 6-10 6z',
  pause: 'M9 6v12M15 6v12',
  alert: 'M12 7v6M12 17v.01',
  branch: 'M7 5v8a4 4 0 0 0 4 4h6M7 5v.01M17 17v.01',
  folder: 'M4 7h6l2 2h8v9H4z',
  settings: 'M5 8h14M5 16h14M9 6v4M15 14v4',
} as const;

export type IconName = keyof typeof PATHS;
export const ICON_NAMES = Object.keys(PATHS) as IconName[];

export interface IconProps extends Omit<SVGProps<SVGSVGElement>, 'name'> {
  name: IconName;
  /** Accessible name; icons without one are decorative and hidden from assistive technology. */
  label?: string;
}

export function Icon({ name, label, ...rest }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={20}
      height={20}
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      role={label ? 'img' : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      {...rest}
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
