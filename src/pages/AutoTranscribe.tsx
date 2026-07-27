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

import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { notifications } from '@mantine/notifications';
import {
  Alert,
  Badge,
  Card,
  Container,
  Group,
  Loader,
  Paper,
  Progress,
  Select,
  SimpleGrid,
  Skeleton,
  Stack,
  Switch,
  Text,
  Title,
} from '@mantine/core';
import { IconInfoCircle, IconMoodCheck } from '@tabler/icons-react';
import { formatDurationLong, formatDurationShort } from '../utils/duration';
import { formatAudioLength, formatFileSize, formatRecordingDate } from '../utils/format';

interface AutoTranscribeStatus {
  enabled: boolean;
  is_running: boolean;
  pending_count: number;
  pending_audio_seconds: number;
  transcribed_count: number;
  transcribed_audio_seconds: number;
  total_words: number;
  seconds_per_audio_hour: number;
  estimated_remaining_seconds: number;
  estimate_basis: string;
  model: string;
  current_file: string | null;
  current_fraction: number;
  activity: string;
  order: string;
  current_slice: Slice | null;
  current_slice_elapsed_seconds: number;
  current_slice_estimated_seconds: number;
}

/** The slice metadata the Slices table carries, for the file in flight. */
interface Slice {
  id: number | null;
  original_audio_file_name: string;
  title: string | null;
  audio_file_size: number;
  audio_file_type: string;
  audio_time_length_seconds: number | null;
  estimated_time_to_transcribe: number;
  recording_date: number | null;
}

const ORDERS = [
  { value: 'oldest', label: 'Oldest recordings first' },
  { value: 'newest', label: 'Newest recordings first' },
  { value: 'shortest', label: 'Shortest first (most files soonest)' },
  { value: 'longest', label: 'Longest first' },
];

const POLL_MS = 2000;

export default function AutoTranscribe() {
  const [status, setStatus] = useState<AutoTranscribeStatus | null>(null);
  const [toggling, setToggling] = useState(false);
  // Held so an in-flight toggle is not clobbered by a poll that started before it.
  const togglingRef = useRef(false);

  const poll = useCallback(async () => {
    try {
      const next = await invoke<AutoTranscribeStatus>('get_auto_transcribe_status');
      setStatus((prev) =>
        togglingRef.current && prev
          ? // Keep the optimistic values a poll that started earlier would undo.
            { ...next, enabled: prev.enabled, order: prev.order }
          : next
      );
    } catch {
      // Backend not ready yet; the next tick will pick it up.
    }
  }, []);

  useEffect(() => {
    poll();
    const interval = setInterval(poll, POLL_MS);
    return () => clearInterval(interval);
  }, [poll]);

  const toggle = async (enabled: boolean) => {
    // Optimistic: the switch should respond instantly, not after a round trip.
    setStatus((prev) => (prev ? { ...prev, enabled } : prev));
    setToggling(true);
    togglingRef.current = true;
    try {
      await invoke('set_auto_transcribe_enabled', { enabled });
      await poll();
    } catch (error) {
      setStatus((prev) => (prev ? { ...prev, enabled: !enabled } : prev));
      notifications.show({
        title: 'Could not change the setting',
        message: String(error),
        color: 'red',
      });
    } finally {
      togglingRef.current = false;
      setToggling(false);
    }
  };

  const changeOrder = async (order: string | null) => {
    if (!order) return;
    setStatus((prev) => (prev ? { ...prev, order } : prev));
    togglingRef.current = true;
    try {
      await invoke('set_auto_transcribe_order', { order });
      togglingRef.current = false;
      await poll();
    } catch (error) {
      togglingRef.current = false;
      await poll();
      notifications.show({
        title: 'Could not change the order',
        message: String(error),
        color: 'red',
      });
    }
  };

  const done = status && status.pending_count === 0;

  return (
    <Container size="lg" py="md">
      <Stack gap="lg">
        <Title order={2}>Auto-Transcribe</Title>

        <Paper p="lg" withBorder radius="md">
          <Group justify="space-between" align="center" wrap="nowrap">
            <Switch
              size="lg"
              checked={status?.enabled ?? false}
              disabled={!status || toggling}
              onChange={(event) => toggle(event.currentTarget.checked)}
              label={
                <Text fw={600} size="md">
                  Continuously transcribe all new audio not yet transcribed
                </Text>
              }
            />
            <Select
              label="Work through the backlog"
              description="Applies to the next file picked up"
              data={ORDERS}
              value={status?.order ?? 'oldest'}
              disabled={!status}
              allowDeselect={false}
              onChange={changeOrder}
              w={280}
            />
          </Group>
        </Paper>

        <Alert icon={<IconInfoCircle size={18} />} color="blue" variant="light">
          Transcription runs at low priority in the background, one file at a time, for as long
          as CiderPress is open. New recordings are picked up automatically every few minutes.
          You can keep working — this is scheduled behind whatever you are doing.
        </Alert>

        <NowTranscribing status={status} />

        <SimpleGrid cols={{ base: 1, sm: 3 }} spacing="md">
          <MetricCard title="Remaining">
            {status ? (
              <>
                <Stat value={status.pending_count.toLocaleString()} label="files to transcribe" />
                <Stat
                  value={formatDurationShort(status.pending_audio_seconds)}
                  label="of audio remaining"
                />
              </>
            ) : (
              <LoadingLines />
            )}
          </MetricCard>

          <MetricCard title="Transcribed">
            {status ? (
              <>
                <Stat value={status.transcribed_count.toLocaleString()} label="files done" />
                <Stat
                  value={formatDurationShort(status.transcribed_audio_seconds)}
                  label="of audio processed"
                />
                <Stat value={status.total_words.toLocaleString()} label="words transcribed" />
                <Stat
                  value={formatDurationShort(status.seconds_per_audio_hour)}
                  label={
                    status.estimate_basis === 'measured'
                      ? 'per hour of audio'
                      : 'per hour of audio (rough)'
                  }
                />
              </>
            ) : (
              <LoadingLines />
            )}
          </MetricCard>

          <MetricCard title="Time to finish">
            {!status ? (
              <LoadingLines />
            ) : done ? (
              <Group gap="xs">
                <IconMoodCheck size={22} />
                <Text fw={600}>Everything is transcribed.</Text>
              </Group>
            ) : (
              <>
                <Text size="xl" fw={700}>
                  {formatDurationLong(status.estimated_remaining_seconds)}
                </Text>
                <Text size="sm" c="dimmed">
                  to finish the remaining {status.pending_count.toLocaleString()} file
                  {status.pending_count === 1 ? '' : 's'} using {status.model}
                </Text>
                {status.estimate_basis !== 'measured' && (
                  <Text size="xs" c="dimmed" mt="xs">
                    This is a rough guess. It will sharpen once more files have been
                    transcribed on this machine.
                  </Text>
                )}
              </>
            )}
          </MetricCard>
        </SimpleGrid>
      </Stack>
    </Container>
  );
}

/** The live strip: what is being transcribed right now, hard to miss. */
function NowTranscribing({ status }: { status: AutoTranscribeStatus | null }) {
  const active = status?.is_running && status.current_file;

  return (
    <Card withBorder radius="md" p="lg">
      {active ? (
        <Stack gap="sm">
          <Group justify="space-between" align="center">
            <Text size="sm" c="dimmed" tt="uppercase" fw={700}>
              Now transcribing
            </Text>
            <Badge variant="light">{Math.round((status.current_fraction ?? 0) * 100)}%</Badge>
          </Group>
          <Text
            size="1.6rem"
            fw={800}
            variant="gradient"
            // Gradient from theme colours so it stays legible across all themes.
            gradient={{ from: 'grape', to: 'cyan', deg: 90 }}
            style={{ wordBreak: 'break-word', lineHeight: 1.2 }}
          >
            {status.current_file}
          </Text>
          <Progress value={(status.current_fraction ?? 0) * 100} animated />
          <CurrentFileMeta status={status} />
        </Stack>
      ) : (
        <Group gap="sm">
          {status?.enabled && <Loader size="xs" color="gray" />}
          <Text size="sm" c="dimmed">
            {status?.enabled
              ? // Says what it is waiting for, not just that it is waiting.
                (status.activity ?? 'Starting up…')
              : 'Auto-transcribe is off. Switch it on to work through the backlog.'}
          </Text>
        </Group>
      )}
    </Card>
  );
}

/**
 * Everything known about the file in flight — the same fields the Slices table
 * carries, plus how far into it we are. Labels are absent by definition: they
 * are derived from the transcript that does not exist yet.
 */
function CurrentFileMeta({ status }: { status: AutoTranscribeStatus }) {
  const slice = status.current_slice;
  if (!slice) return null;

  const remaining = Math.max(
    0,
    status.current_slice_estimated_seconds - status.current_slice_elapsed_seconds
  );

  const fields: [string, string][] = [
    ['Title', slice.title?.trim() || '—'],
    ['Recorded', formatRecordingDate(slice.recording_date)],
    ['Audio length', formatAudioLength(slice.audio_time_length_seconds)],
    ['Size', formatFileSize(slice.audio_file_size)],
    ['Type', slice.audio_file_type || '—'],
    ['Elapsed', formatDurationShort(status.current_slice_elapsed_seconds)],
    ['Left on this file', remaining > 0 ? formatDurationShort(remaining) : 'any moment now'],
  ];

  return (
    <SimpleGrid cols={{ base: 2, sm: 4 }} spacing="xs" verticalSpacing="xs">
      {fields.map(([label, value]) => (
        <div key={label}>
          <Text size="xs" c="dimmed" tt="uppercase" fw={700}>
            {label}
          </Text>
          <Text size="sm" style={{ wordBreak: 'break-word' }}>
            {value}
          </Text>
        </div>
      ))}
    </SimpleGrid>
  );
}

function MetricCard({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <Card withBorder radius="md" p="lg">
      <Text size="sm" c="dimmed" tt="uppercase" fw={700} mb="sm">
        {title}
      </Text>
      <Stack gap="sm">{children}</Stack>
    </Card>
  );
}

function Stat({ value, label }: { value: string; label: string }) {
  return (
    <div>
      <Text size="xl" fw={700}>
        {value}
      </Text>
      <Text size="sm" c="dimmed">
        {label}
      </Text>
    </div>
  );
}

function LoadingLines() {
  return (
    <>
      <Skeleton height={28} width="60%" />
      <Skeleton height={16} width="80%" />
    </>
  );
}
