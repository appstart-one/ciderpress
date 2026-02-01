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
  Text, 
  Stack,
  Grid,
  ThemeIcon,
  Group,
  RingProgress,
  Card,
  SimpleGrid
} from '@mantine/core';
import { BarChart } from '@mantine/charts';
import { IconMicrophone, IconClock, IconFileText, IconTrendingUp, IconX } from '@tabler/icons-react';

interface YearCount {
  year: number;
  count: number;
}

interface AudioLengthBucket {
  label: string;
  count: number;
}

interface Stats {
  total_files: number;
  total_transcribed: number;
  avg_transcribe_sec_10m: number | null;
  total_audio_bytes: number;
  largest_file_bytes: number;
  avg_file_bytes: number;
  count_by_year: YearCount[];
  count_by_audio_length: AudioLengthBucket[];
}

export default function Stats() {
  const [stats, setStats] = useState<Stats | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadStats();
  }, []);

  const loadStats = async () => {
    try {
      const data = await invoke<Stats>('get_stats');
      setStats(data);
    } catch (error) {
      notifications.show({
        title: 'Error',
        message: 'Failed to load statistics',
        color: 'red',
        icon: <IconX size={16} />,
      });
    } finally {
      setLoading(false);
    }
  };

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const formatDuration = (seconds: number | null) => {
    if (seconds === null) return 'N/A';
    if (seconds < 60) return `${seconds.toFixed(1)}s`;
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;
    return `${minutes}m ${remainingSeconds.toFixed(1)}s`;
  };

  const getTranscriptionProgress = () => {
    if (!stats || stats.total_files === 0) return 0;
    return (stats.total_transcribed / stats.total_files) * 100;
  };

  if (loading) {
    return (
      <Container size="md">
        <Text>Loading statistics...</Text>
      </Container>
    );
  }

  if (!stats) {
    return (
      <Container size="md">
        <Text c="red">Failed to load statistics</Text>
      </Container>
    );
  }

  return (
    <Container size="xl">
      <Stack gap="lg">
        <Title order={2}>Statistics</Title>

        {/* Key Metrics */}
        <SimpleGrid cols={{ base: 2, sm: 4 }} spacing="lg">
          <Card withBorder p="lg">
            <Group>
              <ThemeIcon size="xl" variant="light" color="blue">
                <IconMicrophone size={28} />
              </ThemeIcon>
              <div>
                <Text size="xl" fw={700}>{stats.total_files.toLocaleString()}</Text>
                <Text size="sm" c="dimmed">Total Files</Text>
              </div>
            </Group>
          </Card>

          <Card withBorder p="lg">
            <Group>
              <ThemeIcon size="xl" variant="light" color="green">
                <IconFileText size={28} />
              </ThemeIcon>
              <div>
                <Text size="xl" fw={700}>{stats.total_transcribed.toLocaleString()}</Text>
                <Text size="sm" c="dimmed">Transcribed</Text>
              </div>
            </Group>
          </Card>

          <Card withBorder p="lg">
            <Group>
              <ThemeIcon size="xl" variant="light" color="orange">
                <IconClock size={28} />
              </ThemeIcon>
              <div>
                <Text size="xl" fw={700}>{formatDuration(stats.avg_transcribe_sec_10m)}</Text>
                <Text size="sm" c="dimmed">Avg Time/10min</Text>
              </div>
            </Group>
          </Card>

          <Card withBorder p="lg">
            <Group>
              <ThemeIcon size="xl" variant="light" color="violet">
                <IconTrendingUp size={28} />
              </ThemeIcon>
              <div>
                <Text size="xl" fw={700}>{formatBytes(stats.total_audio_bytes)}</Text>
                <Text size="sm" c="dimmed">Total Size</Text>
              </div>
            </Group>
          </Card>
        </SimpleGrid>

        <Grid>
          {/* Transcription Progress */}
          <Grid.Col span={6}>
            <Paper p="lg" withBorder>
              <Stack gap="md">
                <Title order={3}>Transcription Progress</Title>
                <Group justify="center">
                  <RingProgress
                    size={200}
                    thickness={20}
                    sections={[
                      { value: getTranscriptionProgress(), color: 'blue' },
                    ]}
                    label={
                      <div style={{ textAlign: 'center' }}>
                        <Text size="xl" fw={700}>{getTranscriptionProgress().toFixed(1)}%</Text>
                        <Text size="sm" c="dimmed">Complete</Text>
                      </div>
                    }
                  />
                </Group>
                <Text size="sm" ta="center" c="dimmed">
                  {stats.total_transcribed} of {stats.total_files} files transcribed
                </Text>
              </Stack>
            </Paper>
          </Grid.Col>

          {/* Files by Year */}
          <Grid.Col span={6}>
            <Paper p="lg" withBorder>
              <Stack gap="md">
                <Title order={3}>Files by Year</Title>
                {stats.count_by_year.length > 0 ? (
                  <BarChart
                    h={200}
                    data={stats.count_by_year}
                    dataKey="year"
                    series={[{ name: 'count', color: 'blue.6' }]}
                    gridAxis="none"
                  />
                ) : (
                  <Text size="sm" c="dimmed" ta="center">No data available</Text>
                )}
              </Stack>
            </Paper>
          </Grid.Col>

          {/* Files by Audio Length */}
          <Grid.Col span={12}>
            <Paper p="lg" withBorder>
              <Stack gap="md">
                <Title order={3}>Files by Audio Length</Title>
                {stats.count_by_audio_length.length > 0 ? (
                  <BarChart
                    h={200}
                    data={stats.count_by_audio_length}
                    dataKey="label"
                    series={[{ name: 'count', color: 'teal.6' }]}
                    gridAxis="none"
                  />
                ) : (
                  <Text size="sm" c="dimmed" ta="center">No data available</Text>
                )}
              </Stack>
            </Paper>
          </Grid.Col>
        </Grid>

        {/* File Size Information */}
        <Paper p="lg" withBorder>
          <Stack gap="md">
            <Title order={3}>File Size Statistics</Title>
            <SimpleGrid cols={2} spacing="md">
              <div>
                <Text fw={500}>Largest File:</Text>
                <Text size="sm" c="dimmed">
                  {formatBytes(stats.largest_file_bytes)}
                </Text>
              </div>
              <div>
                <Text fw={500}>Average File Size:</Text>
                <Text size="sm" c="dimmed">
                  {formatBytes(stats.avg_file_bytes)}
                </Text>
              </div>
            </SimpleGrid>
          </Stack>
        </Paper>

        {/* Summary Insights */}
        <Paper p="lg" withBorder>
          <Stack gap="md">
            <Title order={3}>Quick Insights</Title>
            <SimpleGrid cols={2} spacing="md">
              <div>
                <Text fw={500}>Most productive year:</Text>
                <Text size="sm" c="dimmed">
                  {stats.count_by_year.length > 0 
                    ? stats.count_by_year.reduce((prev, current) => 
                        prev.count > current.count ? prev : current
                      ).year
                    : 'No data'
                  }
                </Text>
              </div>
              <div>
                <Text fw={500}>Storage efficiency:</Text>
                <Text size="sm" c="dimmed">
                  {stats.total_files > 0 
                    ? `${(stats.total_audio_bytes / stats.total_files / 1024 / 1024).toFixed(1)} MB per recording`
                    : 'No data'
                  }
                </Text>
              </div>
            </SimpleGrid>
          </Stack>
        </Paper>
      </Stack>
    </Container>
  );
} 