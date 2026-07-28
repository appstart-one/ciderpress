import { describe, expect, it } from 'vitest';
import { commitUrl, isDirty, type BuildInfo } from './buildInfo';

function info(over: Partial<BuildInfo> = {}): BuildInfo {
  return {
    version: '0.3.0',
    build_number: '75',
    commit_short: '12c2ec9',
    commit_full: '12c2ec9000000000000000000000000000000000',
    ...over,
  };
}

describe('isDirty', () => {
  it('detects the -dirty marker', () => {
    expect(isDirty(info())).toBe(false);
    expect(isDirty(info({ commit_short: '12c2ec9-dirty' }))).toBe(true);
  });

  it('does not mistake a hash containing "dirty" mid-string', () => {
    // Hex cannot spell "dirty", but the check must be a suffix test regardless.
    expect(isDirty(info({ commit_short: 'dirty123' }))).toBe(false);
  });
});

describe('commitUrl', () => {
  it('builds a GitHub commit URL from the full hash', () => {
    expect(commitUrl(info())).toBe(
      'https://github.com/appstart-one/ciderpress/commit/12c2ec9000000000000000000000000000000000'
    );
  });

  it('returns null when there is no commit to link to', () => {
    // A source tarball has no .git, so build.rs emits placeholders. Linking to
    // /commit/unknown would 404.
    expect(commitUrl(info({ commit_full: 'unknown' }))).toBeNull();
    expect(commitUrl(info({ commit_full: '' }))).toBeNull();
  });

  it('still links a dirty build to its base commit', () => {
    // The binary is not exactly that commit, but it is the right place to look;
    // the "-dirty" suffix in the label carries the caveat.
    const url = commitUrl(info({ commit_short: '12c2ec9-dirty' }));
    expect(url).toContain('/commit/12c2ec9000000000000000000000000000000000');
    expect(url).not.toContain('dirty');
  });
});
