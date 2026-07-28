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

/** Git provenance of the running build, stamped in at compile time by build.rs. */
export interface BuildInfo {
  version: string;
  build_number: string;
  commit_short: string;
  commit_full: string;
}

const REPO = 'https://github.com/appstart-one/ciderpress';

/** True when the build carried uncommitted changes, so the hash is not exact. */
export function isDirty(info: BuildInfo): boolean {
  return info.commit_short.endsWith('-dirty');
}

/**
 * The commit URL for a build, or null when there is nothing to link to.
 *
 * A build made outside a git checkout has no commit, and linking to
 * `/commit/unknown` would 404. A dirty build does not correspond exactly to the
 * commit it names, but the base commit is still the right place to look, so it
 * stays linked and the "-dirty" marker in the label carries the caveat.
 */
export function commitUrl(info: BuildInfo): string | null {
  if (!info.commit_full || info.commit_full === 'unknown') return null;
  return `${REPO}/commit/${info.commit_full}`;
}
