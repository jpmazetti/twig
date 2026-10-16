interface TwigMarkProps {
  size?: number;
}

export function TwigMark({ size = 20 }: TwigMarkProps) {
  return (
    <svg
      aria-hidden="true"
      width={size}
      height={size}
      viewBox="150 150 800 800"
      fill="none"
      stroke="currentColor"
      strokeWidth="58"
      strokeLinecap="round"
    >
      <path d="M430 800 C430 600 560 520 560 270" />
      <path d="M476 599 C540 540 650 500 745 400" />
      <g stroke="none">
        <circle cx="430" cy="800" r="62" fill="currentColor" />
        <circle cx="560" cy="262" r="66" fill="currentColor" />
        <circle cx="748" cy="396" r="56" fill="currentColor" />
      </g>
      <g stroke="none" fill="var(--accent-soft)">
        <circle cx="430" cy="800" r="22" />
        <circle cx="560" cy="262" r="24" />
        <circle cx="748" cy="396" r="20" />
      </g>
    </svg>
  );
}
