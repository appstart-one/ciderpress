import { describe, expect, it } from 'vitest';
import { issueBody, newIssueUrl, MAX_ISSUE_URL_LENGTH } from './issueUrl';
import type { BuildInfo } from './buildInfo';

const build: BuildInfo = {
  version: '0.3.0',
  build_number: '76',
  commit_short: '96f0e50',
  commit_full: '96f0e50aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
};

describe('issueBody', () => {
  it('includes the version and build hash', () => {
    const body = issueBody(build);
    expect(body).toContain('v0.3.0');
    expect(body).toContain('build 76');
    expect(body).toContain('96f0e50');
  });

  it('omits the build line when build info is unavailable', () => {
    const body = issueBody(null);
    expect(body).not.toContain('CiderPress v');
    // The prompts still need to be there — the form is still usable.
    expect(body).toContain('What happened');
  });

  it('prompts for both a bug report and a feature request', () => {
    const body = issueBody(build);
    expect(body).toContain('What happened, or what would you like?');
    expect(body).toContain('Steps to reproduce');
  });
});

describe('newIssueUrl', () => {
  it('points at the repo new-issue form and labels it from-app', () => {
    const url = newIssueUrl(build);
    expect(url.startsWith('https://github.com/appstart-one/ciderpress/issues/new?')).toBe(true);
    expect(url).toContain('labels=from-app');
  });

  it('percent-encodes the body so newlines and markdown survive', () => {
    const url = newIssueUrl(build);
    // Raw newlines or '#' in a query string would truncate or misparse.
    expect(url).not.toMatch(/[\n\r]/);
    expect(url).toContain('%0A'); // encoded newline
    expect(url).toContain('%23'); // encoded '#' from the markdown headings
  });

  it('round-trips back to the exact body', () => {
    const url = newIssueUrl(build);
    const parsed = new URL(url);
    expect(parsed.searchParams.get('body')).toBe(issueBody(build));
  });

  it('stays well under any practical URL length limit', () => {
    // Silent truncation would lose the tail of the template without erroring.
    expect(newIssueUrl(build).length).toBeLessThan(MAX_ISSUE_URL_LENGTH);
  });

  it('carries nothing from the user corpus', () => {
    // The tracker is public and permanent, and this app handles voice memos.
    const body = issueBody(build).toLowerCase();
    for (const leak of ['transcript', '/users/', '.m4a', '.wav', 'recording', 'slice']) {
      expect(body).not.toContain(leak);
    }
  });
});
