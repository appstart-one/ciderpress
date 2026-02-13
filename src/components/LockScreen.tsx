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

import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Box,
  Paper,
  PasswordInput,
  Button,
  Stack,
  Title,
  Text,
  ThemeIcon,
} from '@mantine/core';
import { IconLock, IconShieldLock } from '@tabler/icons-react';

interface LockScreenConfig {
  password_enabled: boolean;
  password_hash: string | null;
  lock_timeout_minutes: number;
}

export function LockScreen({ children }: { children: React.ReactNode }) {
  const [isLocked, setIsLocked] = useState(false);
  const [passwordInput, setPasswordInput] = useState('');
  const [error, setError] = useState('');
  const [config, setConfig] = useState<LockScreenConfig | null>(null);
  const lastActivityRef = useRef(Date.now());
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const loadConfig = useCallback(async () => {
    try {
      const loadedConfig = await invoke<LockScreenConfig>('get_config');
      setConfig(loadedConfig);
      // Lock immediately on first load if password is enabled
      if (loadedConfig.password_enabled && loadedConfig.password_hash) {
        setIsLocked(true);
      }
    } catch {
      // Config not available yet, that's fine
    }
  }, []);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  // Reload config periodically to pick up changes from Settings
  useEffect(() => {
    const interval = setInterval(loadConfig, 5000);
    return () => clearInterval(interval);
  }, [loadConfig]);

  // Track user activity
  const handleActivity = useCallback(() => {
    lastActivityRef.current = Date.now();
  }, []);

  useEffect(() => {
    window.addEventListener('mousemove', handleActivity);
    window.addEventListener('keydown', handleActivity);
    window.addEventListener('click', handleActivity);
    window.addEventListener('scroll', handleActivity);

    return () => {
      window.removeEventListener('mousemove', handleActivity);
      window.removeEventListener('keydown', handleActivity);
      window.removeEventListener('click', handleActivity);
      window.removeEventListener('scroll', handleActivity);
    };
  }, [handleActivity]);

  // Check for inactivity timeout
  useEffect(() => {
    if (!config?.password_enabled || !config?.password_hash || config.lock_timeout_minutes === 0) {
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
      return;
    }

    timerRef.current = setInterval(() => {
      const elapsed = Date.now() - lastActivityRef.current;
      const timeoutMs = config.lock_timeout_minutes * 60 * 1000;
      if (elapsed >= timeoutMs && !isLocked) {
        setIsLocked(true);
        setPasswordInput('');
        setError('');
      }
    }, 1000);

    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current);
      }
    };
  }, [config, isLocked]);

  const hashPassword = async (password: string): Promise<string> => {
    const encoder = new TextEncoder();
    const data = encoder.encode(password);
    const hashBuffer = await crypto.subtle.digest('SHA-256', data);
    const hashArray = Array.from(new Uint8Array(hashBuffer));
    return hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
  };

  const handleUnlock = async () => {
    if (!config?.password_hash) return;

    const hash = await hashPassword(passwordInput);
    if (hash === config.password_hash) {
      setIsLocked(false);
      setPasswordInput('');
      setError('');
      lastActivityRef.current = Date.now();
    } else {
      setError('Incorrect password');
      setPasswordInput('');
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleUnlock();
    }
  };

  if (!isLocked) {
    return <>{children}</>;
  }

  return (
    <Box
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        zIndex: 9999,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: 'var(--mantine-color-body)',
      }}
    >
      <Paper p="xl" withBorder shadow="lg" style={{ width: 360 }}>
        <Stack align="center" gap="lg">
          <ThemeIcon size={64} radius="xl" variant="light" color="blue">
            <IconShieldLock size={36} />
          </ThemeIcon>

          <div style={{ textAlign: 'center' }}>
            <Title order={3}>CiderPress Locked</Title>
            <Text size="sm" c="dimmed" mt={4}>
              Enter your password to unlock
            </Text>
          </div>

          <PasswordInput
            placeholder="Password"
            value={passwordInput}
            onChange={(e) => {
              setPasswordInput(e.target.value);
              setError('');
            }}
            onKeyDown={handleKeyDown}
            error={error || undefined}
            style={{ width: '100%' }}
            autoFocus
          />

          <Button
            fullWidth
            leftSection={<IconLock size={16} />}
            onClick={handleUnlock}
            disabled={!passwordInput}
          >
            Unlock
          </Button>
        </Stack>
      </Paper>
    </Box>
  );
}
