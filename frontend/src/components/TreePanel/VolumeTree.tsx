import { memo, useState } from 'react';
import { useAppStore } from '../../store';
import type { SceneNode } from '../../store/types';

export default function VolumeTree() {
  const sceneGraph = useAppStore((s) => s.sceneGraph);

  if (!sceneGraph) {
    return <div style={{ color: '#666', fontSize: 12 }}>No file loaded</div>;
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
        style={{
          paddingLeft: depth * 12 + 4,
          paddingTop: 2,
          paddingBottom: 2,
          fontSize: 11,
          fontFamily: 'monospace',
          background: isSelected ? '#0f3460' : 'transparent',
          color: isHidden ? '#555' : isSelected ? '#e94560' : '#b0b8c0',
          borderRadius: 2,
          whiteSpace: 'nowrap',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          display: 'flex',
          alignItems: 'center',
        }}
        title={`${node.volume_name} [${node.solid_name}] (${node.material_name})`}
      >
        <button
          className="plain"
          onClick={() => toggleInstanceVisibility(node.instance_id)}
          aria-label={`${isHidden ? 'Show' : 'Hide'} ${node.name}`}
          style={{
            marginRight: 4,
            fontSize: 10,
            opacity: isHidden ? 0.4 : 0.8,
            flexShrink: 0,
          }}
          title={isHidden ? 'Show volume' : 'Hide volume'}
        >
          {isHidden ? '\u{1F441}\u{200D}\u{1F5E8}' : '\u{1F441}'}
        </button>
        <button className="plain" onClick={() => setExpanded(!expanded)} aria-expanded={expanded}
          aria-label={`${expanded ? 'Collapse' : 'Expand'} ${node.name}`}
          style={{ width: '2ch', flexShrink: 0, visibility: node.children.length > 0 ? 'visible' : 'hidden' }}>
          {expanded ? '\u25BC' : '\u25B6'}
        </button>
        <button className="plain" onClick={() => setSelectedVolume(node.volume_name)}
          style={{ flex: 1, minWidth: 0, overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {node.name}
        </button>
      </div>
      {expanded && node.children.map((child, i) => (
        <VolumeNode key={child.instance_id || `${child.volume_name}-${i}`} node={child} depth={depth + 1} />
      ))}
    </div>
  );
});
