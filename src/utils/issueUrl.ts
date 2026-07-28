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

import type { BuildInfo } from './buildInfo';

const REPO = 'https://github.com/appstart-one/ciderpress';

/**
 * Conservative ceiling for the whole URL.
 *
 * Browsers and servers cap URL length in practice and truncate silently rather
 * than erroring, which would lose the tail of the template without telling
 * anyone. Staying well under any real limit costs nothing here.
 */
export const MAX_ISSUE_URL_LENGTH = 2000;

/**
 * The prefilled issue body.
 *
 * Deliberately contains NOTHING drawn from the user's corpus — no transcripts,
 * slice titles, file paths or log contents. This app handles voice memos, and
 * the issue tracker is public and permanent. Build identity is the only thing
 * worth carrying automatically, and it is the one thing users cannot easily
 * report themselves.
 */
export function issueBody(build: BuildInfo | null): string {
  const lines = [
    '### What happened, or what would you like?',
    '',
    '',
    '### Steps to reproduce (skip if this is a feature request)',
    '',
    '1. ',
    '2. ',
    '',
    '---',
  ];

  if (build) {
    lines.push(`CiderPress v${build.version} (build ${build.build_number}, ${build.commit_short})`);
  }

  return lines.join('\n');
}

/**
 * URL for GitHub's new-issue form with the title and body prefilled.
 *
 * Requires the user to be signed in to GitHub — an accepted limitation, chosen
 * over hosting a relay that would need a token and spam defences. The upside is
 * that the user sees exactly what will be posted, in their browser, and can edit
 * or abandon it before anything is submitted.
 */
export function newIssueUrl(build: BuildInfo | null): string {
  const params = new URLSearchParams({
    title: '',
    body: issueBody(build),
    labels: 'from-app',
  });
  // URLSearchParams percent-encodes both keys and values, so newlines and any
  // special characters in the template survive the round trip.
  return `${REPO}/issues/new?${params.toString()}`;
}
