import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('../Viewport/geometryCache', () => ({ clearAllGeometries: vi.fn() }));
import VolumeTree from './VolumeTree';
import { useAppStore } from '../../store';
import type { SceneNode } from '../../store/types';

const node = (name: string, children: SceneNode[] = []): SceneNode => ({
  name, instance_id: `/${name}`, volume_name: name, solid_name: name, material_name: 'M',
  color: null, density: 1, position: [0, 0, 0], rotation: [0, 0, 0], scale: [1, 1, 1], is_world: name === 'W', children,
});

beforeEach(() => {
  useAppStore.setState(useAppStore.getInitialState(), true);
});

describe('VolumeTree', () => {
  it('opens two levels deep, expands on demand and hides a revealed row', async () => {
    useAppStore.setState({ sceneGraph: node('W', [node('A', [node('B', [node('C')])])]) });
    render(<VolumeTree />);
    const user = userEvent.setup();
    expect(screen.getByRole('button', { name: 'Hide B' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Hide C' })).toBeNull();

    await user.click(screen.getByRole('button', { name: 'Expand B' }));
    expect(screen.getByRole('button', { name: 'Collapse B' })).toHaveAttribute('aria-expanded', 'true');
    await user.click(screen.getByRole('button', { name: 'Hide C' }));
    expect(useAppStore.getState().hiddenInstances).toEqual(new Set(['/C']));
    expect(screen.getByRole('button', { name: 'Show C' })).toBeInTheDocument();
  });
});
