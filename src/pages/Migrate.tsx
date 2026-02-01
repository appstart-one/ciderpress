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

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { notifications } from '@mantine/notifications';
import {
  Container,
  Title,
  Paper,
  Button,
  Text,
  Stack,
  Progress,
  Alert,
  Group,
  ThemeIcon,
  SimpleGrid,
  Card,
  Badge,
  Loader
} from '@mantine/core';
import { IconDownload, IconCheck, IconX, IconInfoCircle, IconDatabase, IconFolder, IconCalendar, IconFile, IconFileText } from '@tabler/icons-react';

interface MigrationProgress {
  total_recordings: number;
  processed_recordings: number;
  failed_recordings: number;
  current_recording?: string;
  current_step: string;
  total_size_bytes: number;
  processed_size_bytes: number;
}

interface PreMigrationStats {
  origin_total_files: number;
  origin_total_size_bytes: number;
  origin_most_recent_date: string | null;
  destination_total_files: number;
  destination_most_recent_date: string | null;
  files_to_migrate: number;
  transcribed_count: number;
  not_transcribed_count: number;
}

export default function Migrate() {
  const [isRunning, setIsRunning] = useState(false);
  const [stats, setStats] = useState<MigrationProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [preStats, setPreStats] = useState<PreMigrationStats | null>(null);
  const [loadingPreStats, setLoadingPreStats] = useState(true);

  // Load pre-migration stats on mount
  useEffect(() => {
    loadPreMigrationStats();
  }, []);

  const loadPreMigrationStats = async () => {
    setLoadingPreStats(true);
    try {
      const data = await invoke<PreMigrationStats>('get_pre_migration_stats');
      setPreStats(data);
    } catch (error) {
      console.error('Failed to load pre-migration stats:', error);
    } finally {
      setLoadingPreStats(false);
    }
  };

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const startMigration = async () => {
    console.log("Starting migration...");
    setIsRunning(true);
    setError(null);
    setStats(null);

    try {
      // Open the migration log window
      console.log("Opening migration log window...");
      try {
        await invoke('open_migration_log_window');
        console.log("Migration log window opened successfully");
      } catch (logError) {
        console.warn("Failed to open log window:", logError);
        // Continue with migration even if log window fails
      }
      
      // Start the migration process
      console.log("Invoking start_migration command...");
      await invoke('start_migration');
      console.log("start_migration command completed successfully");
      
      // Poll for progress updates
      const interval = setInterval(async () => {
        try {
          const currentStats = await invoke<MigrationProgress | null>('get_migration_stats');
          console.log("Migration stats:", currentStats);
          setStats(currentStats);
          
          // Check if migration is complete (when currentStats is null, migration is done)
          if (!currentStats || (currentStats.processed_recordings + currentStats.failed_recordings >= currentStats.total_recordings)) {
            clearInterval(interval);
            setIsRunning(false);

            // Reload pre-migration stats to show updated data
            loadPreMigrationStats();

            if (!currentStats) {
              notifications.show({
                title: 'Success',
                message: `Migration completed successfully!`,
                color: 'green',
                icon: <IconCheck size={16} />,
              });
            } else if (currentStats.failed_recordings === 0) {
              notifications.show({
                title: 'Success',
                message: `Migration completed successfully! Processed ${currentStats.processed_recordings} recordings (${(currentStats.total_size_bytes / 1024 / 1024).toFixed(1)}MB).`,
                color: 'green',
                icon: <IconCheck size={16} />,
              });
            } else {
              notifications.show({
                title: 'Migration Complete',
                message: `Migration finished with ${currentStats.failed_recordings} failures out of ${currentStats.total_recordings} recordings.`,
                color: 'yellow',
                icon: <IconInfoCircle size={16} />,
              });
            }
          }
        } catch (error) {
          clearInterval(interval);
          setError(error instanceof Error ? error.message : String(error));
          setIsRunning(false);
          notifications.show({
            title: 'Error',
            message: 'Migration failed',
            color: 'red',
            icon: <IconX size={16} />,
          });
        }
      }, 1000);

    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
      setIsRunning(false);
      notifications.show({
        title: 'Error',
        message: 'Failed to start migration',
        color: 'red',
        icon: <IconX size={16} />,
      });
    }
  };

  const getProgressPercentage = () => {
    if (!stats || stats.total_recordings === 0) return 0;
    return ((stats.processed_recordings + stats.failed_recordings) / stats.total_recordings) * 100;
  };

  return (
    <Container size="md">
      <Stack gap="lg">
        <Title order={2}>Migrate Voice Memos</Title>
        
        <Alert icon={<IconInfoCircle size={16} />} title="Migration Process" color="blue">
          This will copy all your Apple Voice Memos to the CiderPress database, preserving metadata and preparing them for transcription.
        </Alert>

        {/* Pre-Migration Statistics */}
        {loadingPreStats ? (
          <Paper p="lg" withBorder>
            <Group justify="center" p="md">
              <Loader size="sm" />
              <Text size="sm" c="dimmed">Loading statistics...</Text>
            </Group>
          </Paper>
        ) : preStats && (
          <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="md">
            {/* Origin (Apple Voice Memos) Card */}
            <Card shadow="sm" padding="lg" radius="md" withBorder>
              <Group justify="space-between" mb="md">
                <Group gap="sm">
                  <ThemeIcon size="md" variant="light" color="orange">
                    <IconFolder size={18} />
                  </ThemeIcon>
                  <Text fw={600}>Apple Voice Memos</Text>
                </Group>
                <Badge color="orange" variant="light">Origin</Badge>
              </Group>

              <Stack gap="xs">
                <Group justify="space-between">
                  <Group gap="xs">
                    <IconFile size={14} />
                    <Text size="sm" c="dimmed">Total Files</Text>
                  </Group>
                  <Text size="sm" fw={500}>{preStats.origin_total_files}</Text>
                </Group>

                <Group justify="space-between">
                  <Group gap="xs">
                    <IconDatabase size={14} />
                    <Text size="sm" c="dimmed">Total Size</Text>
                  </Group>
                  <Text size="sm" fw={500}>{formatBytes(preStats.origin_total_size_bytes)}</Text>
                </Group>

                <Group justify="space-between">
                  <Group gap="xs">
                    <IconCalendar size={14} />
                    <Text size="sm" c="dimmed">Most Recent</Text>
                  </Group>
                  <Text size="sm" fw={500}>{preStats.origin_most_recent_date || 'N/A'}</Text>
                </Group>
              </Stack>
            </Card>

            {/* Destination (CiderPress) Card */}
            <Card shadow="sm" padding="lg" radius="md" withBorder>
              <Group justify="space-between" mb="md">
                <Group gap="sm">
                  <ThemeIcon size="md" variant="light" color="blue">
                    <IconDatabase size={18} />
                  </ThemeIcon>
                  <Text fw={600}>CiderPress Home</Text>
                </Group>
                <Badge color="blue" variant="light">Destination</Badge>
              </Group>

              <Stack gap="xs">
                <Group justify="space-between">
                  <Group gap="xs">
                    <IconFile size={14} />
                    <Text size="sm" c="dimmed">Total Files</Text>
                  </Group>
                  <Text size="sm" fw={500}>{preStats.destination_total_files}</Text>
                </Group>

                <Group justify="space-between">
                  <Group gap="xs">
                    <IconCalendar size={14} />
                    <Text size="sm" c="dimmed">Most Recent</Text>
                  </Group>
                  <Text size="sm" fw={500}>{preStats.destination_most_recent_date || 'N/A'}</Text>
                </Group>

                <Group justify="space-between">
                  <Group gap="xs">
                    <IconDownload size={14} />
                    <Text size="sm" c="dimmed">Files to Migrate</Text>
                  </Group>
                  <Badge color={preStats.files_to_migrate > 0 ? 'green' : 'gray'} variant="light">
                    {preStats.files_to_migrate}
                  </Badge>
                </Group>

                <Group justify="space-between">
                  <Group gap="xs">
                    <IconFileText size={14} />
                    <Text size="sm" c="dimmed">Transcribed</Text>
                  </Group>
                  <Text size="sm" fw={500}>
                    <Text span c="green" fw={600}>{preStats.transcribed_count}</Text>
                    <Text span c="dimmed"> / </Text>
                    <Text span c="orange" fw={600}>{preStats.not_transcribed_count}</Text>
                    <Text span c="dimmed" size="xs"> pending</Text>
                  </Text>
                </Group>
              </Stack>
            </Card>
          </SimpleGrid>
        )}

        <Paper p="lg" withBorder>
          <Stack gap="md">
            <Group>
              <ThemeIcon size="lg" variant="light" color="blue">
                <IconDatabase size={24} />
              </ThemeIcon>
              <div>
                <Title order={3}>Apple Voice Memos → CiderPress</Title>
                <Text size="sm" c="dimmed">
                  Migrate recordings from Apple's database to your local CiderPress database
                </Text>
              </div>
            </Group>

            {!isRunning && !stats && (
              <Button 
                onClick={startMigration}
                leftSection={<IconDownload size={16} />}
                size="lg"
                fullWidth
              >
                Start Migration
              </Button>
            )}

            {isRunning && (
              <Stack gap="md">
                <Text fw={500}>Migration in Progress...</Text>
                
                {stats && (
                  <>
                    <Text size="sm" c="blue" fw={500}>
                      {stats.current_step}
                    </Text>
                    
                    <Progress 
                      value={getProgressPercentage()} 
                      size="lg"
                      striped
                      animated
                    />
                    
                    <Group justify="space-between">
                      <Text size="sm">
                        Progress: {stats.processed_recordings + stats.failed_recordings} / {stats.total_recordings}
                      </Text>
                      <Text size="sm" c="dimmed">
                        {getProgressPercentage().toFixed(1)}%
                      </Text>
                    </Group>

                    {stats.total_size_bytes > 0 && (
                      <Group justify="space-between">
                        <Text size="sm">
                          Size: {(stats.processed_size_bytes / 1024 / 1024).toFixed(1)}MB / {(stats.total_size_bytes / 1024 / 1024).toFixed(1)}MB
                        </Text>
                        <Text size="sm" c="dimmed">
                          {stats.total_size_bytes > 0 ? ((stats.processed_size_bytes / stats.total_size_bytes) * 100).toFixed(1) : '0.0'}%
                        </Text>
                      </Group>
                    )}

                    {stats.current_recording && (
                      <Text size="sm" c="dimmed">
                        Processing: {stats.current_recording}
                      </Text>
                    )}

                    <Group>
                      <Group gap="xs">
                        <ThemeIcon size="sm" color="green" variant="light">
                          <IconCheck size={12} />
                        </ThemeIcon>
                        <Text size="sm">Success: {stats.processed_recordings}</Text>
                      </Group>
                      
                      {stats.failed_recordings > 0 && (
                        <Group gap="xs">
                          <ThemeIcon size="sm" color="red" variant="light">
                            <IconX size={12} />
                          </ThemeIcon>
                          <Text size="sm">Failed: {stats.failed_recordings}</Text>
                        </Group>
                      )}
                    </Group>
                  </>
                )}
              </Stack>
            )}

            {!isRunning && stats && (
              <Stack gap="md">
                <Alert 
                  icon={<IconCheck size={16} />} 
                  title="Migration Complete" 
                  color={stats.failed_recordings === 0 ? "green" : "yellow"}
                >
                  Successfully processed {stats.processed_recordings} out of {stats.total_recordings} recordings
                  {stats.total_size_bytes > 0 && ` (${(stats.total_size_bytes / 1024 / 1024).toFixed(1)}MB total)`}.
                  {stats.failed_recordings > 0 && ` ${stats.failed_recordings} recordings failed to migrate.`}
                </Alert>

                <Button 
                  onClick={() => {setStats(null); setError(null);}}
                  variant="outline"
                  fullWidth
                >
                  Run Another Migration
                </Button>
              </Stack>
            )}

            {error && (
              <Alert icon={<IconX size={16} />} title="Migration Error" color="red">
                {error}
                <Button 
                  variant="outline" 
                  size="xs" 
                  mt="sm"
                  onClick={() => {setError(null); setStats(null);}}
                >
                  Try Again
                </Button>
              </Alert>
            )}
          </Stack>
        </Paper>

        <Paper p="lg" withBorder>
          <Stack gap="md">
            <Title order={4}>What happens during migration?</Title>
            <Stack gap="xs">
              <Text size="sm">• Creates CiderPress-db.sqlite database in your data directory</Text>
              <Text size="sm">• Copies ZCLOUDRECORDING table from Apple's database</Text>
              <Text size="sm">• Creates slices table for transcription tracking</Text>
              <Text size="sm">• Copies .m4a audio files to CiderPress audio directory</Text>
              <Text size="sm">• Estimates transcription time for each recording</Text>
              <Text size="sm">• Preserves all metadata (title, date, duration, file size)</Text>
              <Text size="sm">• Prepares recordings for transcription workflow</Text>
            </Stack>
          </Stack>
        </Paper>
      </Stack>
    </Container>
  );
} 