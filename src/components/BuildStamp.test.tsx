// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup, act, waitFor } from '@testing-library/react';
import { MantineProvider } from '@mantine/core';

const invokeMock = vi.fn();
// Rejections bypass invokeMock deliberately: vi.fn tracks the settled state of
// promises it returns, and that tracking attaches its own handler, so a rejected
// promise recorded by the spy surfaces as an unhandled rejection and fails the
// run even though the component catches it.
let forceFailure: string | null = null;
let callCount = 0;

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => {
    callCount += 1;
    if (forceFailure) return Promise.reject(new Error(forceFailure));
    return invokeMock(cmd, args);
  },
}));

import BuildStamp from './BuildStamp';

if (!window.matchMedia) {
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
}

const CLEAN = {
  version: '0.3.0',
  build_number: '76',
  commit_short: '96f0e50',
  commit_full: '96f0e50aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
};

function mountWith(info: unknown) {
  invokeMock.mockReset();
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === 'get_build_info') return Promise.resolve(info);
    return Promise.resolve(null);
  });
  return render(
    <MantineProvider>
      <BuildStamp />
    </MantineProvider>
  );
}

describe('BuildStamp', () => {
  beforeEach(() => invokeMock.mockReset());
  afterEach(() => cleanup());

  it('shows version, build number and short hash', async () => {
    mountWith(CLEAN);
    await waitFor(() => expect(screen.getByText('v0.3.0')).toBeTruthy());
    expect(screen.getByText('build 76')).toBeTruthy();
    expect(screen.getByText('96f0e50')).toBeTruthy();
  });

  it('opens the commit on GitHub through open_url, not the webview', async () => {
    mountWith(CLEAN);
    await waitFor(() => expect(screen.getByText('96f0e50')).toBeTruthy());

    await act(async () => {
      screen.getByText('96f0e50').click();
    });

    // Must go through the Tauri command; navigating the webview itself would
    // replace the app with GitHub.
    expect(invokeMock).toHaveBeenCalledWith('open_url', {
      url: `https://github.com/appstart-one/ciderpress/commit/${CLEAN.commit_full}`,
    });
  });

  it('marks a dirty build and still links to the base commit', async () => {
    mountWith({ ...CLEAN, commit_short: '96f0e50-dirty' });
    await waitFor(() => expect(screen.getByText('96f0e50-dirty')).toBeTruthy());

    await act(async () => {
      screen.getByText('96f0e50-dirty').click();
    });
    expect(invokeMock).toHaveBeenCalledWith('open_url', {
      url: `https://github.com/appstart-one/ciderpress/commit/${CLEAN.commit_full}`,
    });
  });

  it('renders the hash unlinked when there is no commit', async () => {
    mountWith({ ...CLEAN, commit_short: 'unknown', commit_full: 'unknown' });
    await waitFor(() => expect(screen.getByText('unknown')).toBeTruthy());

    await act(async () => {
      screen.getByText('unknown').click();
    });
    // Only the initial get_build_info; no open_url for a nonexistent commit.
    expect(invokeMock.mock.calls.filter((c) => c[0] === 'open_url')).toHaveLength(0);
  });

  /** A backend that cannot answer must not put an error where a version goes. */
  it('renders nothing when the command fails', async () => {
    invokeMock.mockReset();
    callCount = 0;
    forceFailure = 'no IPC';
    render(
      <MantineProvider>
        <BuildStamp />
      </MantineProvider>
    );
    await waitFor(() => expect(callCount).toBeGreaterThan(0));

    // Assert on the component's own output, not container.textContent —
    // MantineProvider injects a <style> block, so the container is never empty.
    expect(screen.queryByText(/^v\d/)).toBeNull();
    expect(screen.queryByText(/^build /)).toBeNull();
    forceFailure = null;
  });
});
