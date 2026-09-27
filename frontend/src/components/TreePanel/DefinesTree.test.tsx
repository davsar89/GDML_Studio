import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

vi.mock('../Viewport/geometryCache', () => ({
  clearAllGeometries: vi.fn(),
}));

import DefinesTree from './DefinesTree';
import { useAppStore } from '../../store';
import type { DefineValue, DocumentSummary } from '../../store/types';

/** Far more than any shipped sample: the largest shows 400 scalar defines.
 *  Sized to prove the window holds for a user file well beyond the corpus. */
const HUGE = 22_222;

function makeDefines(n: number): DefineValue[] {
  return Array.from({ length: n }, (_, i) => ({
    name: `d${i}`,
    expression: `${i}`,
    evaluated: i,
    unit: 'mm',
    kind: 'constant',
  }));
}

beforeEach(() => {
  useAppStore.getState().reset();
});

describe('DefinesTree', () => {
  it('mounts a window of rows, not the whole list', () => {
    useAppStore.getState().setDefines(makeDefines(HUGE));
    const { container } = render(<DefinesTree />);

    const rows = container.querySelectorAll('[title]');
    expect(rows.length).toBeGreaterThan(0);
    // jsdom reports clientHeight 0, so the component falls back to a screenful.
    // The point is that it is bounded and nowhere near the full list.
    expect(rows.length).toBeLessThan(200);
    expect(screen.getByText('d0')).toBeInTheDocument();
    expect(screen.queryByText(`d${HUGE - 1}`)).not.toBeInTheDocument();
  });

  it('reaches the middle and final entries using the declared scroll extent', () => {
    useAppStore.getState().setDefines(makeDefines(HUGE));
    const { container } = render(<DefinesTree />);
    const scroller = screen.getByTestId('defines-scroll');
    const spacer = container.querySelector('[data-testid="defines-scroll"] > div') as HTMLElement;
    const extent = parseFloat(spacer.style.height);

    fireEvent.scroll(scroller, { target: { scrollTop: extent / 2 } });
    expect(screen.getByText(`d${Math.floor(HUGE / 2)}`)).toBeInTheDocument();
    expect(screen.queryByText('d0')).not.toBeInTheDocument();
    fireEvent.scroll(scroller, { target: { scrollTop: extent - 1 } });
    expect(screen.getByText(`d${HUGE - 1}`)).toBeInTheDocument();
  });

  it('renders every row when the list is small', () => {
    useAppStore.getState().setDefines(makeDefines(5));
    const { container } = render(<DefinesTree />);
    expect(container.querySelectorAll('[title]').length).toBe(5);
    expect(screen.getByText('d4')).toBeInTheDocument();
  });

  it('does not round tiny values to zero', () => {
    useAppStore.getState().setDefines([
      { name: 'tiny', expression: '1e-9', evaluated: 1e-9, unit: null, kind: 'constant' },
      { name: 'q', expression: '10', evaluated: 10, unit: 'cm', kind: 'quantity' },
      { name: 'third', expression: '1/3', evaluated: 1 / 3, unit: null, kind: 'constant' },
    ]);
    render(<DefinesTree />);
    expect(screen.getByText('1e-9')).toBeInTheDocument();
    expect(screen.getByText('10')).toBeInTheDocument();
    expect(screen.getByText('0.333333')).toBeInTheDocument();
    expect(screen.getByTitle(/tiny = 1e-9 => 1e-9/)).toBeInTheDocument();
  });

  it("shows 'No defines' for a loaded document without defines", () => {
    useAppStore.setState({ summary: { filename: 'x.gdml' } as DocumentSummary });
    render(<DefinesTree />);
    expect(screen.getByText('No defines')).toBeInTheDocument();
  });

  it('shows the empty state with no document', () => {
    render(<DefinesTree />);
    expect(screen.getByText('No file loaded')).toBeInTheDocument();
  });
});
