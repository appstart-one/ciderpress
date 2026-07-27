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

/**
 * Spelled-out duration for the backlog forecast: "1 day, 6 hours, 14 minutes".
 *
 * Leading zero units are omitted — "0 days, 0 hours, 3 minutes" reads like a
 * bug. Anything under a minute collapses to "less than a minute" rather than
 * showing seconds, since the forecast is never that precise.
 */
export function formatDurationLong(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return 'less than a minute';

  const total = Math.round(seconds);
  const days = Math.floor(total / 86400);
  const hours = Math.floor((total % 86400) / 3600);
  const minutes = Math.floor((total % 3600) / 60);

  const parts: string[] = [];
  if (days > 0) parts.push(plural(days, 'day'));
  if (hours > 0) parts.push(plural(hours, 'hour'));
  if (minutes > 0) parts.push(plural(minutes, 'minute'));

  // Non-zero but under a minute, or rounds down to nothing.
  if (parts.length === 0) return 'less than a minute';

  return parts.join(', ');
}

/**
 * Compact duration for audio totals: "41h 12m", "12m", "45s".
 *
 * Hours accumulate rather than rolling into days — "1041h of audio" is a more
 * useful sense of a corpus than "43 days".
 */
export function formatDurationShort(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '0m';

  const total = Math.round(seconds);
  if (total < 60) return `${total}s`;

  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);

  if (hours === 0) return `${minutes}m`;
  return `${hours}h ${minutes}m`;
}

/**
 * Transcription speed as a multiple of real time: "7× real time" means one
 * minute of compute chews through seven minutes of audio.
 *
 * This is the same measurement as "time per hour of audio", turned inside out —
 * a multiplier is the number people can hold in their head. Large factors drop
 * the decimal, because the difference between 34.6× and 35× is noise on a
 * figure averaged over a whole corpus.
 */
export function formatRealtimeFactor(factor: number): string {
  if (!Number.isFinite(factor) || factor <= 0) return 'not measured yet';

  const rounded = factor >= 10 ? Math.round(factor) : Number(factor.toFixed(1));
  // A factor slow enough to round to zero is still not zero, and "0× real
  // time" would read as "nothing is happening".
  if (rounded === 0) return '<0.1× real time';
  return `${rounded}× real time`;
}

function plural(n: number, unit: string): string {
  return `${n} ${unit}${n === 1 ? '' : 's'}`;
}
