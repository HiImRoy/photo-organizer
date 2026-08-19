import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement>;

function IconBase({ children, ...props }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      {...props}
    >
      {children}
    </svg>
  );
}

export function ImportIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M12 3v12" />
      <path d="m7.5 10.5 4.5 4.5 4.5-4.5" />
      <path d="M4 17.5V20h16v-2.5" />
    </IconBase>
  );
}

export function LibraryIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <rect x="3" y="5" width="18" height="15" rx="2.5" />
      <path d="m3 9 4.2-4h4.3l2 2H21" />
      <path d="m7 16 3-3 2.4 2.3 1.8-1.8L18 17" />
    </IconBase>
  );
}

export function HomeIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="m3.5 10.5 8.5-7 8.5 7" />
      <path d="M5.5 9.5v10h13v-10" />
      <path d="M9.5 19.5v-5h5v5" />
    </IconBase>
  );
}

export function BooksIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M5.5 4h12.25A1.25 1.25 0 0 1 19 5.25V8H5.5a2 2 0 0 1 0-4Z" />
      <path d="M4.5 9h12.25A1.25 1.25 0 0 1 18 10.25V13H4.5a2 2 0 0 1 0-4Z" />
      <path d="M5.5 14h12.25A1.25 1.25 0 0 1 19 15.25V18H5.5a2 2 0 0 1 0-4Z" />
      <path d="M8 18v2" />
    </IconBase>
  );
}

export function ImageIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <rect x="3" y="3" width="18" height="18" rx="3" />
      <circle cx="9" cy="9" r="1.5" />
      <path d="m4 18 5-5 3.5 3.5 2-2L20 19" />
    </IconBase>
  );
}

export function SortIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M8 6h12M8 12h8M8 18h4" />
      <path d="M4 5v14m0 0-2.5-2.5M4 19l2.5-2.5" />
    </IconBase>
  );
}

export function SunIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <circle cx="12" cy="12" r="3.5" />
      <path d="M12 2.75v2M12 19.25v2M21.25 12h-2M4.75 12h-2M18.54 5.46l-1.42 1.42M6.88 17.12l-1.42 1.42M18.54 18.54l-1.42-1.42M6.88 6.88 5.46 5.46" />
    </IconBase>
  );
}

export function MoonIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M20 15.2A8.5 8.5 0 0 1 8.8 4 8.5 8.5 0 1 0 20 15.2Z" />
    </IconBase>
  );
}

export function SettingsIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <g transform="translate(4 0)">
        <path d="M9 3.5v1.2a4.8 4.8 0 0 1 1.25.52l1.04-.74 1.23 1.23-.74 1.04c.22.39.4.8.52 1.25h1.2v1.74h-1.2a4.8 4.8 0 0 1-.52 1.25l.74 1.04-1.23 1.23-1.04-.74a4.8 4.8 0 0 1-1.25.52v1.2H7.26v-1.2a4.8 4.8 0 0 1-1.25-.52l-1.04.74-1.23-1.23.74-1.04a4.8 4.8 0 0 1-.52-1.25h-1.2V8h1.2c.12-.45.3-.86.52-1.25l-.74-1.04 1.23-1.23 1.04.74a4.8 4.8 0 0 1 1.25-.52V3.5H9Z" />
        <circle cx="8.13" cy="8.87" r="1.7" />
      </g>
    </IconBase>
  );
}

export function CloseIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="m6 6 12 12M18 6 6 18" />
    </IconBase>
  );
}

export function PauseIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M8 5v14M16 5v14" />
    </IconBase>
  );
}

export function ShieldIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M12 3 5 6v5c0 4.7 2.8 8 7 10 4.2-2 7-5.3 7-10V6z" />
      <path d="m9 12 2 2 4-4" />
    </IconBase>
  );
}

export function ChevronIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="m9 18 6-6-6-6" />
    </IconBase>
  );
}

export function ArrowUpIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M12 19V5" />
      <path d="m6.5 10.5 5.5-5.5 5.5 5.5" />
    </IconBase>
  );
}

export function CheckIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="m5 12 4 4L19 6" />
    </IconBase>
  );
}

export function FilterIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M4 6h16M7 12h10M10 18h4" />
    </IconBase>
  );
}

export function GridIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <rect x="4" y="4" width="6" height="6" rx="1" />
      <rect x="14" y="4" width="6" height="6" rx="1" />
      <rect x="4" y="14" width="6" height="6" rx="1" />
      <rect x="14" y="14" width="6" height="6" rx="1" />
    </IconBase>
  );
}

export function SingleImageIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <rect x="3.5" y="4" width="17" height="16" rx="2" />
      <path d="m5 17 4.5-4.5 3 3 2-2 4.5 4.5" />
    </IconBase>
  );
}

export function SearchIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <circle cx="10.5" cy="10.5" r="6" />
      <path d="m15 15 5 5" />
    </IconBase>
  );
}

export function FolderIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M3 7.5h7l2-2h3.5L18 8.5h3v9.75A1.75 1.75 0 0 1 19.25 20H4.75A1.75 1.75 0 0 1 3 18.25z" />
    </IconBase>
  );
}

export function HeartFolderIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="M3 8h7l2-2h3.5L18 8.5h3v9.75A1.75 1.75 0 0 1 19.25 20H4.75A1.75 1.75 0 0 1 3 18.25z" />
      <path d="m12 16.7-.8-.72C9.35 14.35 8.2 13.3 8.2 11.95A2.1 2.1 0 0 1 12 10.6a2.1 2.1 0 0 1 3.8 1.35c0 1.35-1.15 2.4-3 4.03z" />
    </IconBase>
  );
}

export function PanelIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="M9 4v16" />
    </IconBase>
  );
}

export function PlayIcon(props: IconProps) {
  return (
    <IconBase {...props}>
      <path d="m8 5 11 7-11 7z" />
    </IconBase>
  );
}
