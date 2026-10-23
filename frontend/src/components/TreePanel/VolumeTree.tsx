import { memo, useState } from 'react';
import { useAppStore } from '../../store';
import type { SceneNode } from '../../store/types';

export default function VolumeTree() {
  const sceneGraph = useAppStore((s) => s.sceneGraph);

  if (!sceneGraph) {
    return <div className="empty-state">No file loaded</div>;
  }

  return <VolumeNode node={sceneGraph} depth={0} />;
}

// Memoised with per-node selectors, and collapsed from depth 2 so a large geometry opens as a few rows.
const VolumeNode = memo(({ node, depth }: { node: SceneNode; depth: number }) => {
  const isSelected = useAppStore((s) => s.selectedVolume === node.volume_name);
  const isHidden = useAppStore((s) => s.hiddenInstances.has(node.instance_id));
  const [expanded, setExpanded] = useState(depth < 2);
  const { setSelectedVolume, toggleInstanceVisibility, openContextMenu } = useAppStore.getState();

  return (
    <div>
      <div
        onContextMenu={(e) => {
          e.preventDefault();
          e.stopPropagation();
          openContextMenu(e.clientX, e.clientY, [
            { label: isHidden ? 'Show' : 'Hide', action: () => toggleInstanceVisibility(node.instance_id) },
            { label: 'Change Material', action: () => { setSelectedVolume(node.volume_name); } },
          ]);
        }}
        className="tree-row" data-selected={isSelected} data-hidden={isHidden}
        style={{ paddingLeft: depth * 12 }}
        title={`${node.volume_name} [${node.solid_name}] (${node.material_name})`}
      >
        <button
          className="quiet icon-button"
          onClick={() => toggleInstanceVisibility(node.instance_id)}
          aria-label={`${isHidden ? 'Show' : 'Hide'} ${node.name}`}
          title={isHidden ? 'Show volume' : 'Hide volume'}
        >
          <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z" /><circle cx="12" cy="12" r="3" />
            {isHidden && <path d="m3 3 18 18" />}
          </svg>
        </button>
        <button className="quiet icon-button" onClick={() => setExpanded(!expanded)} aria-expanded={expanded}
          aria-label={`${expanded ? 'Collapse' : 'Expand'} ${node.name}`}
          style={{ visibility: node.children.length > 0 ? 'visible' : 'hidden' }}>
          <span aria-hidden>{expanded ? '▾' : '▸'}</span>
        </button>
        <button className="plain row-name" onClick={() => setSelectedVolume(node.volume_name)} aria-pressed={isSelected}>
          {node.name}
        </button>
      </div>
      {expanded && node.children.map((child) => (
        <VolumeNode key={child.instance_id} node={child} depth={depth + 1} />
      ))}
    </div>
  );
});
