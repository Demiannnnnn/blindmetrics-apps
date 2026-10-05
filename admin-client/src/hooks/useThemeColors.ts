import { useEffect, useState } from 'react';

/**
 * Design tokens resolved to concrete colors for canvas/SVG renderers (ECharts, Leaflet), which
 * cannot use Tailwind classes. Values come from the CSS variables in index.css, so charts follow
 * the same palette and the light/dark theme of the rest of the app.
 */
export interface ThemeColors {
  bg: string;
  surface: string;
  surfaceSubtle: string;
  border: string;
  borderStrong: string;
  text: string;
  textMuted: string;
  accent: string;
  success: string;
  warning: string;
  danger: string;
  info: string;
}

const TOKENS: Record<keyof ThemeColors, string> = {
  bg: 'bg',
  surface: 'surface',
  surfaceSubtle: 'surface-subtle',
  border: 'border',
  borderStrong: 'border-strong',
  text: 'text',
  textMuted: 'text-muted',
  accent: 'accent',
  success: 'success',
  warning: 'warning',
  danger: 'danger',
  info: 'info',
};

/**
 * Light-theme values: fallback when the stylesheet is not available (e.g. in tests), and fixed
 * palette for surfaces that are always light (map tiles, Leaflet popups).
 */
export const LIGHT_THEME_COLORS: ThemeColors = {
  bg: '#fafafa',
  surface: '#ffffff',
  surfaceSubtle: '#f5f5f5',
  border: '#e5e5e5',
  borderStrong: '#d4d4d4',
  text: '#0a0a0a',
  textMuted: '#525252',
  accent: '#dc2626',
  success: '#15803d',
  warning: '#b45309',
  danger: '#b91c1c',
  info: '#1d4ed8',
};

export function readThemeColors(): ThemeColors {
  if (typeof window === 'undefined') return LIGHT_THEME_COLORS;
  const style = getComputedStyle(document.documentElement);
  const colors = { ...LIGHT_THEME_COLORS };
  for (const [key, token] of Object.entries(TOKENS) as [keyof ThemeColors, string][]) {
    const value = style.getPropertyValue(`--${token}`).trim();
    if (value) colors[key] = value;
  }
  return colors;
}

/** `#rrggbb` + alpha (0–1) → `rgba(...)`; other formats are returned unchanged. */
export function withAlpha(color: string, alpha: number): string {
  const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(color);
  if (!m) return color;
  const [r, g, b] = [m[1], m[2], m[3]].map((h) => parseInt(h ?? '0', 16));
  return `rgba(${String(r)}, ${String(g)}, ${String(b)}, ${String(alpha)})`;
}

/** Theme colors that update when the system switches between light and dark. */
export function useThemeColors(): ThemeColors {
  const [colors, setColors] = useState<ThemeColors>(readThemeColors);
  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const update = () => setColors(readThemeColors());
    media.addEventListener('change', update);
    return () => media.removeEventListener('change', update);
  }, []);
  return colors;
}
