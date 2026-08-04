import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('./Viewport/geometryCache', () => ({
  clearAllGeometries: vi.fn(),
}));

import ErrorBoundary from './ErrorBoundary';
import { useAppStore } from '../store';

/** Throws while the store holds a document, renders fine once it is cleared. */
function Boom() {
  const materials = useAppStore((s) => s.materials);
  if (materials.length > 0) throw new Error('render exploded');
  return <div>recovered</div>;
}

beforeEach(() => {
  // React logs the caught error; keep the test output readable.
  vi.spyOn(console, 'error').mockImplementation(() => {});
  useAppStore.getState().reset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('ErrorBoundary', () => {
  it('shows the fallback instead of unmounting the tree', () => {
    useAppStore.getState().setMaterials([{ name: 'G4_AIR' } as never]);

    render(
      <ErrorBoundary>
        <Boom />
      </ErrorBoundary>,
    );

    expect(screen.getByText('Something went wrong')).toBeInTheDocument();
    expect(screen.getByText('render exploded')).toBeInTheDocument();
  });

  it('recovers when "Try again" is clicked', async () => {
    const user = userEvent.setup();
    useAppStore.getState().setMaterials([{ name: 'G4_AIR' } as never]);

    render(
      <ErrorBoundary>
        <Boom />
      </ErrorBoundary>,
    );
    expect(screen.getByText('Something went wrong')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Try again' }));

    // Clearing `error` alone would re-render the same children against the same
    // store state that threw, and the boundary would catch again immediately.
    // Retry has to drop the document first for this to be reachable at all.
    expect(screen.getByText('recovered')).toBeInTheDocument();
    expect(screen.queryByText('Something went wrong')).not.toBeInTheDocument();
    expect(useAppStore.getState().materials).toEqual([]);
  });
});
