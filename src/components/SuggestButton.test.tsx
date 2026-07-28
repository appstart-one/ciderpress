// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup, act, waitFor } from '@testing-library/react';
import { MantineProvider } from '@mantine/core';

const invokeMock = vi.fn();
let callCount = 0;
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => {
    callCount += 1;
    return invokeMock(cmd, args);
  },
}));

import SuggestButton from './SuggestButton';
import { issueBody } from '../utils/issueUrl';

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

const BUILD = {
  version: '0.3.0',
  build_number: '76',
  commit_short: '96f0e50',
  commit_full: '96f0e50aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
};

function mount(buildInfo: unknown = BUILD) {
  invokeMock.mockReset();
  callCount = 0;
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === 'get_build_info') return Promise.resolve(buildInfo);
    return Promise.resolve(null);
  });
  return render(
    <MantineProvider>
      <SuggestButton />
    </MantineProvider>
  );
}

function openUrlArg(): string | undefined {
  const call = invokeMock.mock.calls.find((c) => c[0] === 'open_url');
  return (call?.[1] as { url?: string } | undefined)?.url;
}

describe('SuggestButton', () => {
  beforeEach(() => invokeMock.mockReset());
  afterEach(() => cleanup());

  it('is labelled so the purpose is obvious', () => {
    mount();
    expect(screen.getByText('Suggest a feature or fix')).toBeTruthy();
  });

  it('opens the prefilled GitHub issue form in the browser', async () => {
    mount();
    await waitFor(() => expect(callCount).toBeGreaterThan(0));

    await act(async () => {
      screen.getByText('Suggest a feature or fix').click();
    });

    const url = openUrlArg();
    expect(url).toBeTruthy();
    // Goes out through open_url; navigating the webview would replace the app.
    expect(url).toContain('https://github.com/appstart-one/ciderpress/issues/new?');
    expect(url).toContain('labels=from-app');
  });

  it('stamps the running build into the report', async () => {
    mount();
    await waitFor(() => expect(callCount).toBeGreaterThan(0));
    await act(async () => {
      screen.getByText('Suggest a feature or fix').click();
    });

    const body = new URL(openUrlArg()!).searchParams.get('body') ?? '';
    expect(body).toContain('v0.3.0');
    expect(body).toContain('build 76');
    expect(body).toContain('96f0e50');
    expect(body).toBe(issueBody(BUILD));
  });

  /** Build info is a nicety; the button must not become unusable without it. */
  it('still opens a usable form when build info is unavailable', async () => {
    mount(null);
    await waitFor(() => expect(callCount).toBeGreaterThan(0));
    await act(async () => {
      screen.getByText('Suggest a feature or fix').click();
    });

    const body = new URL(openUrlArg()!).searchParams.get('body') ?? '';
    expect(body).toContain('What happened');
    expect(body).not.toContain('CiderPress v');
  });
});
