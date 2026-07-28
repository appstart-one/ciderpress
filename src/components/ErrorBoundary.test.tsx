// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup, act } from '@testing-library/react';
import { MemoryRouter, Routes, Route, useNavigate } from 'react-router-dom';
import { MantineProvider } from '@mantine/core';
import { ErrorBoundary, RoutedErrorBoundary } from './ErrorBoundary';

// jsdom has no matchMedia, and MantineProvider calls it on mount. Minimal stub
// rather than a full polyfill dependency — nothing here asserts on media queries.
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

/** Throws on render when asked to, so a boundary has something to catch. */
function Bomb({ explode }: { explode: boolean }) {
  if (explode) throw new Error('kaboom from the bomb');
  return <div>bomb is inert</div>;
}

function GoTo({ to, label }: { to: string; label: string }) {
  const navigate = useNavigate();
  return <button onClick={() => navigate(to)}>{label}</button>;
}

function wrap(ui: React.ReactNode, initial = '/') {
  return render(
    <MantineProvider>
      <MemoryRouter initialEntries={[initial]}>{ui}</MemoryRouter>
    </MantineProvider>
  );
}

describe('RoutedErrorBoundary', () => {
  // React logs caught render errors to console.error; silence it so a passing
  // run is not full of expected stack traces.
  let spy: ReturnType<typeof vi.spyOn>;
  beforeEach(() => {
    spy = vi.spyOn(console, 'error').mockImplementation(() => {});
  });
  afterEach(() => {
    spy.mockRestore();
    cleanup();
  });

  /**
   * The actual bug: one page throwing made every other page show the same
   * fallback, because a single boundary instance persisted across navigation.
   */
  it('clears the fallback when the route changes', async () => {
    wrap(
      <>
        <GoTo to="/safe" label="go safe" />
        <RoutedErrorBoundary>
          <Routes>
            <Route path="/" element={<Bomb explode />} />
            <Route path="/safe" element={<div>safe page content</div>} />
          </Routes>
        </RoutedErrorBoundary>
      </>
    );

    // The broken route shows the fallback.
    expect(screen.getByText('Something went wrong')).toBeTruthy();

    await act(async () => {
      screen.getByText('go safe').click();
    });

    // The other route renders its own content, not the inherited error.
    expect(screen.getByText('safe page content')).toBeTruthy();
    expect(screen.queryByText('Something went wrong')).toBeNull();
  });

  /**
   * Guards the regression in the other direction: keying on pathname must not
   * remount on same-route renders, or "Try Again" could never work.
   */
  it('keeps the fallback while the route is unchanged', async () => {
    function Rerender() {
      const navigate = useNavigate();
      // Navigating to the path we are already on must not reset the boundary.
      return <button onClick={() => navigate('/')}>renavigate</button>;
    }

    wrap(
      <>
        <Rerender />
        <RoutedErrorBoundary>
          <Routes>
            <Route path="/" element={<Bomb explode />} />
          </Routes>
        </RoutedErrorBoundary>
      </>
    );

    expect(screen.getByText('Something went wrong')).toBeTruthy();
    await act(async () => {
      screen.getByText('renavigate').click();
    });
    expect(screen.getByText('Something went wrong')).toBeTruthy();
  });

  /** Acceptance item 5: the error UI itself is unchanged. */
  it('renders the message, the stack and a Try Again button', () => {
    render(
      <MantineProvider>
        <ErrorBoundary>
          <Bomb explode />
        </ErrorBoundary>
      </MantineProvider>
    );

    expect(screen.getByText('Something went wrong')).toBeTruthy();
    expect(screen.getByText('kaboom from the bomb')).toBeTruthy();
    expect(screen.getByText('Try Again')).toBeTruthy();
    expect(
      screen.getByText('An error occurred while rendering this page.')
    ).toBeTruthy();
  });

  /** Acceptance item 2: same-route recovery still works. */
  it('Try Again recovers the current route in place', async () => {
    // Gate on an external flag, not a render counter: React re-invokes a
    // failing render in development to capture the stack, so a counter-based
    // component "recovers" before the fallback is ever shown.
    let shouldThrow = true;
    function Recoverable() {
      if (shouldThrow) throw new Error('one-shot failure');
      return <div>recovered content</div>;
    }

    render(
      <MantineProvider>
        <ErrorBoundary>
          <Recoverable />
        </ErrorBoundary>
      </MantineProvider>
    );

    expect(screen.getByText('Something went wrong')).toBeTruthy();

    // Whatever was broken is now fixed; Try Again should show the page.
    shouldThrow = false;
    await act(async () => {
      screen.getByText('Try Again').click();
    });
    expect(screen.getByText('recovered content')).toBeTruthy();
    expect(screen.queryByText('Something went wrong')).toBeNull();
  });
});
