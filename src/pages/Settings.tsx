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

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { notifications } from '@mantine/notifications';
import {
  Container,
  Title,
  Paper,
  TextInput,
  PasswordInput,
  NumberInput,
  Button,
  Group,
  Text,
  Stack,
  Alert,
  Select,
  Progress,
  Switch,
  Box,
  Transition,
  ThemeIcon
} from '@mantine/core';
import { IconCheck, IconX, IconInfoCircle, IconDownload, IconShieldLock, IconLock } from '@tabler/icons-react';
import { DraggableCard } from '../components/DraggableCard';

interface ModelDownloadProgress {
  model_name: string;
  percentage: number;
  status: string;
  error_message: string | null;
}

interface Config {
  voice_memo_root: string;
  ciderpress_home: string;
  model_name: string;
  first_run_complete: boolean;
  skip_already_transcribed: boolean;
  password_enabled: boolean;
  password_hash: string | null;
  lock_timeout_minutes: number;
}

// Base Whisper model information
const WHISPER_MODEL_INFO: Record<string, { label: string; size: string }> = {
  'tiny': { label: 'Tiny', size: '~39 MB' },
  'tiny.en': { label: 'Tiny English', size: '~39 MB' },
  'base': { label: 'Base', size: '~74 MB' },
  'base.en': { label: 'Base English', size: '~74 MB' },
  'small': { label: 'Small', size: '~244 MB' },
  'small.en': { label: 'Small English', size: '~244 MB' },
  'medium': { label: 'Medium', size: '~769 MB' },
  'medium.en': { label: 'Medium English', size: '~769 MB' },
  'large': { label: 'Large', size: '~1550 MB' },
  'large-v1': { label: 'Large v1', size: '~1550 MB' },
  'large-v2': { label: 'Large v2', size: '~1550 MB' },
  'large-v3': { label: 'Large v3', size: '~1550 MB' },
  'large-v3-turbo': { label: 'Large v3 Turbo', size: '~809 MB' },
};

export default function Settings() {
  const [config, setConfig] = useState<Config>({
    voice_memo_root: '',
    ciderpress_home: '',
    model_name: 'base.en',
    first_run_complete: false,
    skip_already_transcribed: true,
    password_enabled: false,
    password_hash: null,
    lock_timeout_minutes: 5,
  });
  const [isLoading, setIsLoading] = useState(false);
  const [isValid, setIsValid] = useState<boolean>(false);
  const [loading, setLoading] = useState(true);
  const [downloadedModels, setDownloadedModels] = useState<string[]>([]);
  const [downloadProgress, setDownloadProgress] = useState<ModelDownloadProgress | null>(null);
  const [isDownloading, setIsDownloading] = useState(false);
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [passwordError, setPasswordError] = useState('');

  useEffect(() => {
    loadConfig();
  }, []);

  // Listen for model download progress events
  useEffect(() => {
    const unlisten = listen<ModelDownloadProgress>('model-download-progress', (event) => {
      const progress = event.payload;
      setDownloadProgress(progress);

      if (progress.status === 'completed') {
        setIsDownloading(false);
        setDownloadProgress(null);
        // Refresh the downloaded models list
        loadDownloadedModels();
        notifications.show({
          title: 'Download Complete',
          message: `Model ${progress.model_name} downloaded successfully`,
          color: 'green',
          icon: <IconCheck size={16} />,
        });
      } else if (progress.status === 'error') {
        setIsDownloading(false);
        setDownloadProgress(null);
        notifications.show({
          title: 'Download Failed',
          message: progress.error_message || 'Unknown error',
          color: 'red',
          icon: <IconX size={16} />,
        });
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const loadConfig = async () => {
    try {
      const loadedConfig = await invoke<Config>('get_config');
      setConfig(loadedConfig);
      await validatePaths();
      await loadDownloadedModels();
    } catch (error) {
      notifications.show({
        title: 'Error',
        message: 'Failed to load configuration',
        color: 'red',
        icon: <IconX size={16} />,
      });
    } finally {
      setLoading(false);
    }
  };

  const loadDownloadedModels = async () => {
    try {
      const downloaded = await invoke<string[]>('get_downloaded_models');
      setDownloadedModels(downloaded);
    } catch (error) {
      console.error('Failed to load downloaded models:', error);
    }
  };

  // Build whisper model options with download status icons
  const getWhisperModelOptions = () => {
    return Object.entries(WHISPER_MODEL_INFO).map(([value, info]) => {
      const isDownloaded = downloadedModels.includes(value);
      return {
        value,
        label: `${info.label} (${info.size})${isDownloaded ? ' ✓' : ''}`,
      };
    });
  };

  const isCurrentModelDownloaded = downloadedModels.includes(config.model_name);

  const downloadModel = async () => {
    if (isDownloading) return;

    setIsDownloading(true);
    try {
      await invoke('download_whisper_model', { modelName: config.model_name });
    } catch (error) {
      setIsDownloading(false);
      notifications.show({
        title: 'Download Failed',
        message: String(error),
        color: 'red',
        icon: <IconX size={16} />,
      });
    }
  };

  const validatePaths = async () => {
    try {
      const valid = await invoke<boolean>('validate_paths');
      setIsValid(valid);
    } catch (error) {
      setIsValid(false);
      notifications.show({
        title: 'Error',
        message: 'Failed to validate paths',
        color: 'red',
        icon: <IconX size={16} />,
      });
    }
  };

  const saveConfig = async () => {
    setIsLoading(true);

    try {
      await invoke('update_config', { newConfig: config });
      await validatePaths();
      notifications.show({
        title: 'Success',
        message: 'Configuration saved successfully',
        color: 'green',
        icon: <IconCheck size={16} />,
      });
    } catch (error) {
      notifications.show({
        title: 'Error',
        message: 'Failed to save configuration',
        color: 'red',
        icon: <IconX size={16} />,
      });
    } finally {
      setIsLoading(false);
    }
  };

  const hashPassword = async (password: string): Promise<string> => {
    const encoder = new TextEncoder();
    const data = encoder.encode(password);
    const hashBuffer = await crypto.subtle.digest('SHA-256', data);
    const hashArray = Array.from(new Uint8Array(hashBuffer));
    return hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
  };

  const handlePasswordToggle = async (enabled: boolean) => {
    if (enabled) {
      // Turning on - require password to be set first
      setConfig({ ...config, password_enabled: true });
    } else {
      // Turning off - clear password
      setNewPassword('');
      setConfirmPassword('');
      setPasswordError('');
      setConfig({ ...config, password_enabled: false, password_hash: null });
    }
  };

  const handleSetPassword = async () => {
    if (newPassword.length < 4) {
      setPasswordError('Password must be at least 4 characters');
      return;
    }
    if (newPassword !== confirmPassword) {
      setPasswordError('Passwords do not match');
      return;
    }
    const hash = await hashPassword(newPassword);
    setConfig({ ...config, password_hash: hash });
    setNewPassword('');
    setConfirmPassword('');
    setPasswordError('');
    notifications.show({
      title: 'Password Set',
      message: 'Remember to save configuration to apply changes',
      color: 'blue',
      icon: <IconLock size={16} />,
    });
  };

  const resetToDefaults = () => {
    const defaultConfig: Config = {
      voice_memo_root: '/Users/yourname/Library/Group Containers/group.com.apple.VoiceMemos.shared/Recordings',
      ciderpress_home: '/Users/yourname/.ciderpress',
      model_name: 'base.en',
      first_run_complete: false,
      skip_already_transcribed: true,
      password_enabled: false,
      password_hash: null,
      lock_timeout_minutes: 5,
    };
    setConfig(defaultConfig);
    setIsValid(false);
    notifications.show({
      title: 'Success',
      message: 'Configuration reset to defaults',
      color: 'blue',
      icon: <IconInfoCircle size={16} />,
    });
  };

  if (loading) {
    return (
      <Container size="md">
        <Text>Loading configuration...</Text>
      </Container>
    );
  }

  return (
    <Container size="md">
      <Stack gap="lg">
        <Title order={2}>Settings</Title>
        
        <Alert icon={<IconInfoCircle size={16} />} title="Configuration" color="blue">
          Configure the paths and settings for CiderPress Voice Memo Liberator.
        </Alert>

        <Paper p="lg" withBorder>
          <Stack gap="md">
            <Title order={3}>Paths</Title>
            
            <TextInput
              label="Voice Memo Root Directory"
              description="Path to Apple's Voice Memos directory (contains CloudRecordings.db)"
              placeholder="/Users/yourname/Library/Group Containers/group.com.apple.VoiceMemos.shared/Recordings"
              value={config.voice_memo_root}
              onChange={(e) => setConfig({ ...config, voice_memo_root: e.target.value })}
              required
            />

            <TextInput
              label="CiderPress Home Directory"
              description="Directory where CiderPress stores its data and database"
              placeholder="/Users/yourname/.ciderpress"
              value={config.ciderpress_home}
              onChange={(e) => setConfig({ ...config, ciderpress_home: e.target.value })}
              required
            />
          </Stack>
        </Paper>

        <Paper p="lg" withBorder>
          <Stack gap="md">
            <Title order={3}>Transcription</Title>

            <Select
              label="Whisper Model"
              description="Whisper model to use for transcription (larger models are more accurate but slower). Models marked with ✓ are already downloaded."
              value={config.model_name}
              onChange={(value) => setConfig({ ...config, model_name: value || 'base.en' })}
              data={getWhisperModelOptions()}
              searchable
              required
              disabled={isDownloading}
            />
            <Text size="xs" c="dimmed">
              Recommended: Large v3 Turbo offers the best balance of speed, accuracy, and size. On an M2 Mac, expect roughly 30 seconds to transcribe 10 minutes of audio.
            </Text>

            {!isCurrentModelDownloaded && !isDownloading && (
              <Group>
                <Button
                  variant="light"
                  leftSection={<IconDownload size={16} />}
                  onClick={downloadModel}
                >
                  Download Model
                </Button>
                <Text size="sm" c="dimmed">
                  This model needs to be downloaded before use ({WHISPER_MODEL_INFO[config.model_name]?.size})
                </Text>
              </Group>
            )}

            {isDownloading && downloadProgress && (
              <Text size="sm" c="blue">Downloading model... (see progress popup)</Text>
            )}

            {isCurrentModelDownloaded && !isDownloading && (
              <Text size="sm" c="green">
                <IconCheck size={14} style={{ verticalAlign: 'middle', marginRight: 4 }} />
                Model ready to use
              </Text>
            )}

            <Switch
              label="Skip already transcribed slices"
              description="When enabled, slices that have already been transcribed will be skipped. Disable to re-transcribe selected slices."
              checked={config.skip_already_transcribed}
              onChange={(e) => setConfig({ ...config, skip_already_transcribed: e.currentTarget.checked })}
            />
          </Stack>
        </Paper>

        <Paper p="lg" withBorder>
          <Stack gap="md">
            <Title order={3}>Security</Title>

            <Switch
              label="Enable password lock"
              description="When enabled, the app will lock after a period of inactivity and require a password to unlock."
              checked={config.password_enabled}
              onChange={(e) => handlePasswordToggle(e.currentTarget.checked)}
            />

            {config.password_enabled && (
              <>
                <PasswordInput
                  label="Password"
                  description="Enter a password to protect the app"
                  placeholder="Enter password"
                  value={newPassword}
                  onChange={(e) => {
                    setNewPassword(e.target.value);
                    setPasswordError('');
                  }}
                  error={passwordError && !confirmPassword ? passwordError : undefined}
                />

                <PasswordInput
                  label="Confirm Password"
                  description="Re-enter the password to confirm"
                  placeholder="Confirm password"
                  value={confirmPassword}
                  onChange={(e) => {
                    setConfirmPassword(e.target.value);
                    setPasswordError('');
                  }}
                  error={passwordError || undefined}
                />

                <Button
                  variant="light"
                  leftSection={<IconLock size={16} />}
                  onClick={handleSetPassword}
                  disabled={!newPassword || !confirmPassword}
                >
                  Set Password
                </Button>

                {config.password_hash && (
                  <Text size="sm" c="green">
                    <IconCheck size={14} style={{ verticalAlign: 'middle', marginRight: 4 }} />
                    Password is set
                  </Text>
                )}

                <NumberInput
                  label="Lock timeout (minutes)"
                  description="Number of minutes of inactivity before the app locks. Set to 0 to only lock on app restart."
                  value={config.lock_timeout_minutes}
                  onChange={(value) => setConfig({ ...config, lock_timeout_minutes: typeof value === 'number' ? value : 5 })}
                  min={0}
                  max={120}
                  step={1}
                />
              </>
            )}
          </Stack>
        </Paper>

        <Paper p="lg" withBorder>
          <Stack gap="md">
            <Title order={3}>Status</Title>
            <Group>
              <div style={{
                width: 12,
                height: 12,
                borderRadius: '50%',
                backgroundColor: isValid ? '#10b981' : '#ef4444'
              }} />
              <Text size="sm" c={isValid ? 'green' : 'red'}>
                {isValid ? 'Apple DB connection successful' : 'Cannot connect to Apple DB'}
              </Text>
            </Group>
            {!isValid && !loading && (
              <Alert icon={<IconShieldLock size={16} />} title="Cannot Access Voice Memos" color="red" variant="light">
                <Stack gap="xs">
                  <Text size="sm">
                    CiderPress cannot read the Apple Voice Memos directory. This is usually caused by one of two things:
                  </Text>
                  <Text size="sm" fw={600}>Option 1: Grant Full Disk Access (recommended)</Text>
                  <Text size="sm">
                    Open <b>System Settings &rarr; Privacy &amp; Security &rarr; Full Disk Access</b> and enable CiderPress.
                    Then restart the app.
                  </Text>
                  <Text size="sm" fw={600}>Option 2: Copy files manually</Text>
                  <Text size="sm">
                    Copy your Voice Memos from <code>~/Library/Group Containers/group.com.apple.VoiceMemos.shared/Recordings/</code> to a folder you can access, then update the <b>Voice Memo Root Directory</b> path above to point there.
                  </Text>
                </Stack>
              </Alert>
            )}
          </Stack>
        </Paper>

        <Group justify="space-between">
          <Button variant="outline" onClick={resetToDefaults}>
            Reset to Defaults
          </Button>

          <Group>
            <Button variant="outline" onClick={loadConfig}>
              Reload
            </Button>
            <Button
              onClick={saveConfig}
              loading={isLoading}
              leftSection={<IconCheck size={16} />}
            >
              Save Configuration
            </Button>
          </Group>
        </Group>

        {/* Draggable Model Download Progress Popup */}
        <Transition mounted={isDownloading && downloadProgress !== null} transition="slide-up" duration={400}>
          {(styles) => (
            <Box
              style={{
                ...styles,
                position: 'fixed',
                bottom: 24,
                left: '50%',
                transform: 'translateX(-50%)',
                zIndex: 1000,
                width: 'min(400px, calc(100vw - 48px))',
              }}
            >
              <DraggableCard shadow="xl" padding="lg" radius="lg" withBorder style={{ background: 'var(--mantine-color-body)' }}>
                <Stack gap="md">
                  <Group gap="sm">
                    <ThemeIcon size="lg" radius="md" variant="light" color="blue">
                      <IconDownload size={20} />
                    </ThemeIcon>
                    <div>
                      <Text fw={600} size="sm">Downloading Model</Text>
                      <Text size="xs" c="dimmed">
                        {downloadProgress?.model_name}
                      </Text>
                    </div>
                  </Group>
                  <Progress
                    value={downloadProgress?.percentage || 0}
                    size="lg"
                    radius="xl"
                    striped
                    animated
                  />
                  <Text size="xs" c="dimmed" ta="center">
                    {downloadProgress?.percentage.toFixed(1)}%
                  </Text>
                </Stack>
              </DraggableCard>
            </Box>
          )}
        </Transition>
      </Stack>
    </Container>
  );
} 