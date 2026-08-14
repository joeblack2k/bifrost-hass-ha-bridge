import type { ReactNode, SVGProps } from 'react'

export type IconName =
  | 'arrow-right'
  | 'check'
  | 'chevron-down'
  | 'chevron-left'
  | 'chevron-right'
  | 'circle'
  | 'cloud'
  | 'grid'
  | 'lamp'
  | 'layers'
  | 'link'
  | 'list'
  | 'log'
  | 'minus'
  | 'plus'
  | 'refresh'
  | 'room'
  | 'save'
  | 'search'
  | 'settings'
  | 'sliders'
  | 'spark'
  | 'square'
  | 'trash'
  | 'x'

export function Icon({ name, size = 18, ...props }: { name: IconName; size?: number } & SVGProps<SVGSVGElement>) {
  const common = {
    fill: 'none',
    stroke: 'currentColor',
    strokeLinecap: 'round' as const,
    strokeLinejoin: 'round' as const,
    strokeWidth: 1.8,
  }

  let content: ReactNode
  switch (name) {
    case 'arrow-right':
      content = <path d="M4 12h15m-6-6 6 6-6 6" />
      break
    case 'check':
      content = <path d="m5 12 4.5 4.5L19 7" />
      break
    case 'chevron-down':
      content = <path d="m6 9 6 6 6-6" />
      break
    case 'chevron-left':
      content = <path d="m14 6-6 6 6 6" />
      break
    case 'chevron-right':
      content = <path d="m10 6 6 6-6 6" />
      break
    case 'circle':
      content = <circle cx="12" cy="12" r="8" />
      break
    case 'cloud':
      content = <path d="M7.5 18h9.75a4.75 4.75 0 0 0 .5-9.47A6.25 6.25 0 0 0 5.8 10.9 3.6 3.6 0 0 0 7.5 18Z" />
      break
    case 'grid':
      content = <><rect x="4" y="4" width="6" height="6" rx="1" /><rect x="14" y="4" width="6" height="6" rx="1" /><rect x="4" y="14" width="6" height="6" rx="1" /><rect x="14" y="14" width="6" height="6" rx="1" /></>
      break
    case 'lamp':
      content = <><path d="M8 4h8l2 8H6l2-8Z" /><path d="M12 12v5m-3 3h6" /></>
      break
    case 'layers':
      content = <><path d="m12 4 8 4-8 4-8-4 8-4Z" /><path d="m4 12 8 4 8-4M4 16l8 4 8-4" /></>
      break
    case 'link':
      content = <><path d="m10 13.5-1 1a3.5 3.5 0 1 1-5-5l2-2a3.5 3.5 0 0 1 5 0" /><path d="m14 10.5 1-1a3.5 3.5 0 1 1 5 5l-2 2a3.5 3.5 0 0 1-5 0" /><path d="m8 16 8-8" /></>
      break
    case 'list':
      content = <><path d="M8 6h12M8 12h12M8 18h12" /><circle cx="4" cy="6" r=".7" fill="currentColor" /><circle cx="4" cy="12" r=".7" fill="currentColor" /><circle cx="4" cy="18" r=".7" fill="currentColor" /></>
      break
    case 'log':
      content = <><path d="M5 4h14v16H5z" /><path d="M8 8h8M8 12h8M8 16h5" /></>
      break
    case 'minus':
      content = <path d="M5 12h14" />
      break
    case 'plus':
      content = <path d="M12 5v14M5 12h14" />
      break
    case 'refresh':
      content = <><path d="M19 8a7.5 7.5 0 0 0-13.1-1.7L4 8.5" /><path d="M4 4.5v4h4" /><path d="M5 16a7.5 7.5 0 0 0 13.1 1.7l1.9-2.2" /><path d="M20 19.5v-4h-4" /></>
      break
    case 'room':
      content = <><path d="M4 20V5.5L12 3l8 2.5V20" /><path d="M8 20v-5h8v5M8 9h.01M12 8h.01M16 9h.01" /></>
      break
    case 'save':
      content = <><path d="M5 4h11l3 3v13H5z" /><path d="M8 4v5h7V4M8 20v-7h8v7" /></>
      break
    case 'search':
      content = <><circle cx="10.8" cy="10.8" r="6.3" /><path d="m16 16 4 4" /></>
      break
    case 'settings':
      content = <><circle cx="12" cy="12" r="3" /><path d="m19.4 15 .1.1a1.7 1.7 0 0 1-2.4 2.4l-.1-.1a1.7 1.7 0 0 0-2.9 1.2v.2a1.7 1.7 0 0 1-3.4 0v-.2a1.7 1.7 0 0 0-2.9-1.2l-.1.1a1.7 1.7 0 1 1-2.4-2.4l.1-.1A1.7 1.7 0 0 0 6.2 12a1.7 1.7 0 0 0-1.2-2.9h-.2a1.7 1.7 0 0 1 0-3.4H5A1.7 1.7 0 0 0 6.2 2.8l-.1-.1a1.7 1.7 0 1 1 2.4-2.4l.1.1A1.7 1.7 0 0 0 11.5 1.2h.2a1.7 1.7 0 0 1 3.4 0v.2A1.7 1.7 0 0 0 18 2.6l.1-.1a1.7 1.7 0 1 1 2.4 2.4l-.1.1A1.7 1.7 0 0 0 21.6 8v.2a1.7 1.7 0 0 1 0 3.4h-.2a1.7 1.7 0 0 0-1.2 2.9Z" transform="translate(0 0) scale(.78) translate(3.4 3.4)" /></>
      break
    case 'sliders':
      content = <><path d="M4 6h16M4 12h16M4 18h16" /><circle cx="9" cy="6" r="2" fill="currentColor" stroke="none" /><circle cx="15" cy="12" r="2" fill="currentColor" stroke="none" /><circle cx="11" cy="18" r="2" fill="currentColor" stroke="none" /></>
      break
    case 'spark':
      content = <path d="m12 3 1.5 6.5L20 12l-6.5 1.5L12 20l-1.5-6.5L4 12l6.5-2.5L12 3Z" />
      break
    case 'square':
      content = <rect x="5" y="5" width="14" height="14" rx="2" />
      break
    case 'trash':
      content = <><path d="M5 7h14M10 11v5M14 11v5M8 7l1-3h6l1 3m-9 0 1 13h10l1-13" /></>
      break
    case 'x':
      content = <path d="m6 6 12 12M18 6 6 18" />
      break
  }

  return (
    <svg
      aria-hidden="true"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      {...common}
      {...props}
    >
      {content}
    </svg>
  )
}
