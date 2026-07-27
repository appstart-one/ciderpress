import { describe, expect, it } from 'vitest';
import { formatDurationLong, formatDurationShort } from './duration';

describe('formatDurationLong', () => {
  it('omits leading zero units', () => {
    expect(formatDurationLong(180)).toBe('3 minutes');
    expect(formatDurationLong(3600)).toBe('1 hour');
    expect(formatDurationLong(86400)).toBe('1 day');
  });

  it('handles the unit boundaries', () => {
    expect(formatDurationLong(0)).toBe('less than a minute');
    expect(formatDurationLong(59)).toBe('less than a minute');
    expect(formatDurationLong(60)).toBe('1 minute');
    expect(formatDurationLong(3599)).toBe('59 minutes');
    expect(formatDurationLong(86399)).toBe('23 hours, 59 minutes');
  });

  it('spells out multi-day forecasts', () => {
    // 1 day, 6 hours, 14 minutes
    expect(formatDurationLong(86400 + 6 * 3600 + 14 * 60)).toBe('1 day, 6 hours, 14 minutes');
    expect(formatDurationLong(3 * 86400 + 2 * 3600)).toBe('3 days, 2 hours');
  });

  it('never renders a zero-only string', () => {
    for (const s of [0, 1, 30, 59, 60, 3599, 3600, 86399, 86400, 500000]) {
      expect(formatDurationLong(s)).not.toMatch(/\b0 (day|hour|minute)/);
    }
  });

  it('survives garbage input', () => {
    expect(formatDurationLong(NaN)).toBe('less than a minute');
    expect(formatDurationLong(-5)).toBe('less than a minute');
    expect(formatDurationLong(Infinity)).toBe('less than a minute');
  });
});

describe('formatDurationShort', () => {
  it('handles the unit boundaries', () => {
    expect(formatDurationShort(0)).toBe('0m');
    expect(formatDurationShort(59)).toBe('59s');
    expect(formatDurationShort(60)).toBe('1m');
    expect(formatDurationShort(3599)).toBe('59m');
    expect(formatDurationShort(3600)).toBe('1h 0m');
    expect(formatDurationShort(86399)).toBe('23h 59m');
  });

  it('accumulates hours instead of rolling into days', () => {
    expect(formatDurationShort(86400)).toBe('24h 0m');
    expect(formatDurationShort(41 * 3600 + 12 * 60)).toBe('41h 12m');
  });
});
