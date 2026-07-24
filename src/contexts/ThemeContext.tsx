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

import React, { createContext, useContext, useState, useEffect } from 'react';
import {
  themes,
  resolveTheme,
  DEFAULT_LIGHT_THEME,
  DEFAULT_DARK_THEME,
  type ThemeBase,
} from '../themes';

const THEME_KEY = 'ciderpress-theme';
const LEGACY_KEY = 'ciderpress-color-scheme';

interface ThemeContextType {
  /** Active theme id (e.g. 'dracula'). */
  themeId: string;
  /** Switch to any theme in the registry. */
  setThemeId: (id: string) => void;
  /** Derived base ('light' | 'dark') of the active theme. */
  colorScheme: ThemeBase;
  /** Navbar quick toggle: flip between the default Light and Dark themes. */
  toggleColorScheme: () => void;
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined);

export const useTheme = () => {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error('useTheme must be used within a ThemeProvider');
  }
  return context;
};

/**
 * Determine the initial theme id:
 *   1. New 'ciderpress-theme' key if present and valid.
 *   2. Migrate legacy 'ciderpress-color-scheme' ('light' | 'dark').
 *   3. Fall back to system preference.
 * Wrapped in try/catch so malformed storage never throws on boot.
 */
function getInitialThemeId(): string {
  try {
    const saved = localStorage.getItem(THEME_KEY);
    if (saved && themes[saved]) {
      return saved;
    }

    // Migrate the legacy light/dark toggle value.
    const legacy = localStorage.getItem(LEGACY_KEY);
    if (legacy === 'light' || legacy === 'dark') {
      return legacy;
    }

    if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
      return DEFAULT_DARK_THEME;
    }
  } catch {
    // Ignore storage/matchMedia failures and use the default below.
  }
  return DEFAULT_LIGHT_THEME;
}

interface ThemeProviderProps {
  children: React.ReactNode;
}

export const ThemeProvider: React.FC<ThemeProviderProps> = ({ children }) => {
  const [themeId, setThemeIdState] = useState<string>(getInitialThemeId);

  useEffect(() => {
    try {
      localStorage.setItem(THEME_KEY, themeId);
      // Keep the legacy key roughly in sync so an older build (or the lock
      // screen) still reads a sensible light/dark value.
      localStorage.setItem(LEGACY_KEY, resolveTheme(themeId).base);
    } catch {
      // Storage may be unavailable (private mode) — non-fatal.
    }
  }, [themeId]);

  const setThemeId = (id: string) => {
    setThemeIdState(themes[id] ? id : DEFAULT_LIGHT_THEME);
  };

  const colorScheme = resolveTheme(themeId).base;

  const toggleColorScheme = () => {
    setThemeIdState(colorScheme === 'dark' ? DEFAULT_LIGHT_THEME : DEFAULT_DARK_THEME);
  };

  return (
    <ThemeContext.Provider value={{ themeId, setThemeId, colorScheme, toggleColorScheme }}>
      {children}
    </ThemeContext.Provider>
  );
};
