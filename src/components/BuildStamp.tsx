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

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Anchor, Group, Text, Tooltip } from '@mantine/core';
import { type BuildInfo, commitUrl, isDirty } from '../utils/buildInfo';

/**
 * Version, build number and commit hash, small and dimmed at the bottom of the
 * navbar. Metadata, not chrome: it must never compete with the navigation.
 *
 * Exists so a bug report can name exactly which build produced it. The build
 * number is the commit count, so it is comparable across machines.
 */
export default function BuildStamp() {
  const [info, setInfo] = useState<BuildInfo | null>(null);

  useEffect(() => {
    // Static for the life of the process — fetch once, never poll.
    invoke<BuildInfo>('get_build_info')
      .then(setInfo)
      .catch(() => {
        // Nothing to show is better than an error where a version belongs.
      });
  }, []);

  if (!info) return null;

  const url = commitUrl(info);
  const dirty = isDirty(info);

  return (
    <Group gap={6} justify="flex-start" wrap="nowrap" pt="xs">
      <Text size="xs" c="dimmed">
        v{info.version}
      </Text>
      <Text size="xs" c="dimmed">
        ·
      </Text>
      <Text size="xs" c="dimmed">
        build {info.build_number}
      </Text>
      <Text size="xs" c="dimmed">
        ·
      </Text>
      <Tooltip
        label={
          dirty
            ? 'Built with uncommitted changes — links to the last commit'
            : 'View this commit on GitHub'
        }
        withArrow
        disabled={!url}
      >
        {url ? (
          <Anchor
            size="xs"
            c="dimmed"
            style={{ fontFamily: 'monospace' }}
            onClick={(e) => {
              e.preventDefault();
              invoke('open_url', { url }).catch(() => {});
            }}
            href={url}
          >
            {info.commit_short}
          </Anchor>
        ) : (
          <Text size="xs" c="dimmed" style={{ fontFamily: 'monospace' }}>
            {info.commit_short}
          </Text>
        )}
      </Tooltip>
    </Group>
  );
}
