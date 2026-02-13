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

import { BrowserRouter as Router, Routes, Route, Link, useLocation } from 'react-router-dom';
import { AppShell, NavLink, Title, Group, ThemeIcon } from '@mantine/core';
import { IconSettings, IconDownload, IconChartBar, IconDatabase, IconTags, IconNotebook, IconLock } from '@tabler/icons-react';
import { ThemeToggle } from './components/ThemeToggle';
import { ErrorBoundary } from './components/ErrorBoundary';
import { LockScreen, useLockScreen } from './components/LockScreen';
import Settings from './pages/Settings';
import Migrate from './pages/Migrate';
import Stats from './pages/Stats';
import Slices from './pages/Slices';
import LabelSettings from './pages/LabelSettings';
import NotebookLM from './pages/NotebookLM';

function Navigation() {
  const location = useLocation();
  
  const navItems = [
    { path: '/', label: 'Slices', icon: IconDatabase },
    { path: '/stats', label: 'Stats', icon: IconChartBar },
    { path: '/migrate', label: 'Migrate', icon: IconDownload },
    { path: '/labels', label: 'Labels', icon: IconTags },
    { path: '/notebook-lm', label: 'NotebookLM', icon: IconNotebook },
    { path: '/settings', label: 'Settings', icon: IconSettings },
  ];

  return (
    <>
      {navItems.map((item) => (
        <NavLink
          key={item.path}
          component={Link}
          to={item.path}
          label={item.label}
          leftSection={
            <ThemeIcon variant="light" size="sm">
              <item.icon size={16} />
            </ThemeIcon>
          }
          active={location.pathname === item.path}
        />
      ))}
    </>
  );
}

function LockNowButton() {
  const { lockNow, isPasswordEnabled } = useLockScreen();

  if (!isPasswordEnabled) return null;

  return (
    <NavLink
      label="Lock Now"
      leftSection={
        <ThemeIcon variant="light" size="sm" color="red">
          <IconLock size={16} />
        </ThemeIcon>
      }
      onClick={lockNow}
      style={{ borderTop: '1px solid var(--mantine-color-default-border)' }}
      pt="sm"
    />
  );
}

function App() {
  return (
    <Router>
      <LockScreen>
        <AppShell
          navbar={{
            width: 250,
            breakpoint: 'sm',
          }}
          padding="md"
        >
          <AppShell.Navbar p="md">
            <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
              <Group justify="space-between" mb="md">
                <Title order={3}>CiderPress</Title>
                <ThemeToggle />
              </Group>
              <div style={{ flex: 1 }}>
                <Navigation />
              </div>
              <LockNowButton />
            </div>
          </AppShell.Navbar>

          <AppShell.Main>
            <ErrorBoundary>
              <Routes>
                <Route path="/" element={<Slices />} />
                <Route path="/migrate" element={<Migrate />} />
                <Route path="/stats" element={<Stats />} />
                <Route path="/settings" element={<Settings />} />
                <Route path="/labels" element={<LabelSettings />} />
                <Route path="/notebook-lm" element={<NotebookLM />} />
              </Routes>
            </ErrorBoundary>
          </AppShell.Main>
        </AppShell>
      </LockScreen>
    </Router>
  );
}

export default App; 