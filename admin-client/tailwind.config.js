/** @type {import('tailwindcss').Config} */
// Semantic tokens of the ecosystem design system (values in src/index.css, same as telemetry-web).
const token = (name) => `var(--${name})`;

export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        bg: token('bg'),
        surface: { DEFAULT: token('surface'), subtle: token('surface-subtle') },
        default: token('border'),
        strong: token('border-strong'),
        fg: token('text'),
        muted: token('text-muted'),
        accent: {
          DEFAULT: token('accent'),
          hover: token('accent-hover'),
          fg: token('accent-fg'),
          subtle: token('accent-subtle'),
        },
        success: { DEFAULT: token('success'), subtle: token('success-subtle') },
        warning: { DEFAULT: token('warning'), subtle: token('warning-subtle') },
        danger: {
          DEFAULT: token('danger'),
          hover: token('danger-hover'),
          subtle: token('danger-subtle'),
        },
        info: { DEFAULT: token('info'), subtle: token('info-subtle') },
        ink: {
          DEFAULT: token('ink'),
          raised: token('ink-raised'),
          line: token('ink-line'),
          fg: token('ink-fg'),
          muted: token('ink-muted'),
        },
        // Red that doesn't change with the theme, for marks on ink surfaces
        brand: { DEFAULT: '#dc2626', hover: '#b91c1c' },
      },
      fontFamily: {
        sans: ['"IBM Plex Sans"', 'ui-sans-serif', 'system-ui', 'sans-serif'],
        mono: ['"IBM Plex Mono"', 'ui-monospace', 'monospace'],
      },
      borderRadius: {
        control: '4px',
        card: '6px',
        badge: '3px',
      },
    },
  },
  plugins: [],
};
