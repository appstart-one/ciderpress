// VoiceMemoLiberator - Voice memo transcription and management tool
// Copyright (C) 2026 APPSTART LLC
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

import {
  createTheme,
  type CSSVariablesResolver,
  type MantineColorsTuple,
  type MantineThemeOverride,
} from '@mantine/core';

// ---------------------------------------------------------------------------
// Theme registry
// ---------------------------------------------------------------------------
// A theme is a self-contained bundle of:
//   • base       — 'light' | 'dark', drives MantineProvider forceColorScheme
//   • mantine    — a Mantine theme override (primaryColor + generated palettes)
//   • cssVars    — a cssVariablesResolver that recolors *surfaces* (body, cards,
//                  borders, text) so Dracula/Nord/etc. restyle the whole app and
//                  not just the accent color.
//   • swatch     — two representative hexes (surface + accent) for the picker UI
// Light and Dark are the stock Mantine defaults (empty surface overrides).
// ---------------------------------------------------------------------------

export type ThemeBase = 'light' | 'dark';

export interface AppTheme {
  id: string;
  label: string;
  base: ThemeBase;
  /** Two-tone preview: base surface + accent. */
  swatch: { surface: string; accent: string };
  mantine: MantineThemeOverride;
  cssVariables: CSSVariablesResolver;
}

// --- color math -----------------------------------------------------------

function clamp(v: number): number {
  return Math.max(0, Math.min(255, Math.round(v)));
}

function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace('#', '');
  const full = h.length === 3
    ? h.split('').map((c) => c + c).join('')
    : h;
  return [
    parseInt(full.slice(0, 2), 16),
    parseInt(full.slice(2, 4), 16),
    parseInt(full.slice(4, 6), 16),
  ];
}

function rgbToHex(r: number, g: number, b: number): string {
  return `#${[r, g, b].map((c) => clamp(c).toString(16).padStart(2, '0')).join('')}`;
}

/** Blend `hex` toward `target` by `amount` (0..1). */
function mix(hex: string, target: string, amount: number): string {
  const [r1, g1, b1] = hexToRgb(hex);
  const [r2, g2, b2] = hexToRgb(target);
  return rgbToHex(
    r1 + (r2 - r1) * amount,
    g1 + (g2 - g1) * amount,
    b1 + (b2 - b1) * amount,
  );
}

const WHITE = '#ffffff';
const BLACK = '#000000';

/**
 * Build a 10-shade Mantine tuple centered on `base` at index 6.
 * Indices 0-5 tint toward white, 7-9 shade toward black. With primaryShade
 * pinned to 6 (below) the accent renders exactly as the official palette hex.
 */
function shades(base: string): MantineColorsTuple {
  return [
    mix(base, WHITE, 0.82),
    mix(base, WHITE, 0.66),
    mix(base, WHITE, 0.49),
    mix(base, WHITE, 0.33),
    mix(base, WHITE, 0.18),
    mix(base, WHITE, 0.08),
    base,
    mix(base, BLACK, 0.16),
    mix(base, BLACK, 0.3),
    mix(base, BLACK, 0.44),
  ] as unknown as MantineColorsTuple;
}

// --- surface override helper ----------------------------------------------

interface Surfaces {
  body: string;    // app background
  surface: string; // card / input / --mantine-color-default
  hover: string;   // --mantine-color-default-hover
  border: string;  // --mantine-color-default-border
  text: string;    // --mantine-color-text
  dimmed: string;  // --mantine-color-dimmed
}

/** Resolver that writes surface overrides into the block matching `base`. */
function surfaceResolver(base: ThemeBase, s: Surfaces): CSSVariablesResolver {
  const vars = {
    '--mantine-color-body': s.body,
    '--mantine-color-text': s.text,
    '--mantine-color-dimmed': s.dimmed,
    '--mantine-color-default': s.surface,
    '--mantine-color-default-hover': s.hover,
    '--mantine-color-default-border': s.border,
  };
  return () => ({
    variables: {},
    light: base === 'light' ? vars : {},
    dark: base === 'dark' ? vars : {},
  });
}

const emptyResolver: CSSVariablesResolver = () => ({ variables: {}, light: {}, dark: {} });

const FONT = '-apple-system, BlinkMacSystemFont, Segoe UI, Roboto, sans-serif';

interface CustomThemeInput {
  id: string;
  label: string;
  base: ThemeBase;
  accent: string;
  surfaces: Surfaces;
}

function customTheme(input: CustomThemeInput): AppTheme {
  const { id, label, base, accent, surfaces } = input;
  return {
    id,
    label,
    base,
    swatch: { surface: surfaces.body, accent },
    mantine: createTheme({
      fontFamily: FONT,
      primaryColor: id,
      primaryShade: 6,
      autoContrast: true,
      colors: {
        [id]: shades(accent),
      },
    }),
    cssVariables: surfaceResolver(base, surfaces),
  };
}

// --- the 12 themes --------------------------------------------------------

export const themes: Record<string, AppTheme> = {
  light: {
    id: 'light',
    label: 'Light',
    base: 'light',
    swatch: { surface: '#ffffff', accent: '#228be6' },
    mantine: createTheme({ fontFamily: FONT, primaryColor: 'blue' }),
    cssVariables: emptyResolver,
  },
  dark: {
    id: 'dark',
    label: 'Dark',
    base: 'dark',
    swatch: { surface: '#1a1b1e', accent: '#228be6' },
    mantine: createTheme({ fontFamily: FONT, primaryColor: 'blue' }),
    cssVariables: emptyResolver,
  },
  dracula: customTheme({
    id: 'dracula',
    label: 'Dracula',
    base: 'dark',
    accent: '#bd93f9', // Dracula purple
    surfaces: {
      body: '#282a36',
      surface: '#343746',
      hover: '#44475a',
      border: '#44475a',
      text: '#f8f8f2',
      dimmed: '#6272a4',
    },
  }),
  nord: customTheme({
    id: 'nord',
    label: 'Nord',
    base: 'dark',
    accent: '#88c0d0', // Nord frost
    surfaces: {
      body: '#2e3440',
      surface: '#3b4252',
      hover: '#434c5e',
      border: '#4c566a',
      text: '#d8dee9',
      dimmed: '#7b88a1',
    },
  }),
  'solarized-dark': customTheme({
    id: 'solarized-dark',
    label: 'Solarized Dark',
    base: 'dark',
    accent: '#268bd2', // Solarized blue
    surfaces: {
      body: '#002b36',
      surface: '#073642',
      hover: '#094451',
      border: '#586e75',
      text: '#93a1a1',
      dimmed: '#657b83',
    },
  }),
  'solarized-light': customTheme({
    id: 'solarized-light',
    label: 'Solarized Light',
    base: 'light',
    accent: '#268bd2', // Solarized blue
    surfaces: {
      body: '#fdf6e3',
      surface: '#eee8d5',
      hover: '#e3ddc9',
      border: '#93a1a1',
      text: '#586e75',
      dimmed: '#657b83',
    },
  }),
  'gruvbox-dark': customTheme({
    id: 'gruvbox-dark',
    label: 'Gruvbox Dark',
    base: 'dark',
    accent: '#fabd2f', // Gruvbox yellow
    surfaces: {
      body: '#282828',
      surface: '#3c3836',
      hover: '#504945',
      border: '#504945',
      text: '#ebdbb2',
      dimmed: '#a89984',
    },
  }),
  'tokyo-night': customTheme({
    id: 'tokyo-night',
    label: 'Tokyo Night',
    base: 'dark',
    accent: '#7aa2f7', // Tokyo Night blue
    surfaces: {
      body: '#1a1b26',
      surface: '#24283b',
      hover: '#292e42',
      border: '#292e42',
      text: '#c0caf5',
      dimmed: '#565f89',
    },
  }),
  'catppuccin-mocha': customTheme({
    id: 'catppuccin-mocha',
    label: 'Catppuccin Mocha',
    base: 'dark',
    accent: '#cba6f7', // Catppuccin mauve
    surfaces: {
      body: '#1e1e2e',
      surface: '#313244',
      hover: '#45475a',
      border: '#45475a',
      text: '#cdd6f4',
      dimmed: '#a6adc8',
    },
  }),
  'rose-pine': customTheme({
    id: 'rose-pine',
    label: 'Rosé Pine',
    base: 'dark',
    accent: '#c4a7e7', // Rosé Pine iris
    surfaces: {
      body: '#191724',
      surface: '#1f1d2e',
      hover: '#26233a',
      border: '#403d52',
      text: '#e0def4',
      dimmed: '#908caa',
    },
  }),
  'one-dark': customTheme({
    id: 'one-dark',
    label: 'One Dark',
    base: 'dark',
    accent: '#61afef', // One Dark blue
    surfaces: {
      body: '#282c34',
      surface: '#2c313c',
      hover: '#3b4048',
      border: '#3e4451',
      text: '#abb2bf',
      dimmed: '#5c6370',
    },
  }),
  monokai: customTheme({
    id: 'monokai',
    label: 'Monokai',
    base: 'dark',
    accent: '#a6e22e', // Monokai green
    surfaces: {
      body: '#272822',
      surface: '#3e3d32',
      hover: '#49483e',
      border: '#49483e',
      text: '#f8f8f2',
      dimmed: '#75715e',
    },
  }),
};

/** Ordered list for rendering the picker grid. */
export const themeList: AppTheme[] = [
  themes.light,
  themes.dark,
  themes.dracula,
  themes.nord,
  themes['solarized-dark'],
  themes['solarized-light'],
  themes['gruvbox-dark'],
  themes['tokyo-night'],
  themes['catppuccin-mocha'],
  themes['rose-pine'],
  themes['one-dark'],
  themes.monokai,
];

export const DEFAULT_LIGHT_THEME = 'light';
export const DEFAULT_DARK_THEME = 'dark';

/** Resolve a possibly-unknown id to a valid theme, falling back to Light. */
export function resolveTheme(id: string | null | undefined): AppTheme {
  if (id && themes[id]) {
    return themes[id];
  }
  return themes.light;
}
