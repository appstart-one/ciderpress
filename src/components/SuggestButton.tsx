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
import { NavLink, ThemeIcon, Tooltip } from '@mantine/core';
import { IconMessagePlus } from '@tabler/icons-react';
import type { BuildInfo } from '../utils/buildInfo';
import { newIssueUrl } from '../utils/issueUrl';

/**
 * Opens GitHub's new-issue form with a template already filled in.
 *
 * Takes the user to their browser rather than posting on their behalf, which
 * means they need a GitHub account — an accepted tradeoff, chosen over hosting a
 * relay with a token and spam defences. It also means they see exactly what will
 * be published before anything is submitted, which matters for an app that
 * handles voice memos and files into a public, permanent tracker.
 *
 * The tooltip states both facts, because "Suggest a feature" gives no hint that
 * you are about to leave the app and hit a sign-in wall.
 */
export default function SuggestButton() {
  const [build, setBuild] = useState<BuildInfo | null>(null);

  useEffect(() => {
    // Only used to stamp the report; a failure just omits that line.
    invoke<BuildInfo>('get_build_info')
      .then(setBuild)
      .catch(() => {});
  }, []);

  return (
    <Tooltip
      label="Opens GitHub in your browser — you'll need a GitHub account to post"
      withArrow
      multiline
      w={240}
      position="right"
    >
      <NavLink
        label="Suggest a feature or fix"
        leftSection={
          <ThemeIcon variant="light" size="sm" color="grape">
            <IconMessagePlus size={16} />
          </ThemeIcon>
        }
        onClick={() => {
          invoke('open_url', { url: newIssueUrl(build) }).catch(() => {});
        }}
      />
    </Tooltip>
  );
}
