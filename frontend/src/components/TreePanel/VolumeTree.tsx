import { useAppStore } from '../../store';
import type { SceneNode } from '../../store/types';

export default function VolumeTree() {
  const sceneGraph = useAppStore((s) => s.sceneGraph);

  if (!sceneGraph) {
    return <div style={{ color: '#666', fontSize: 12 }}>No file loaded</div>;
  }

  return <VolumeNode node={sceneGraph} depth={0} />;
}

function VolumeNode({ node, depth }: { node: SceneNode; depth: number }) {
  const selectedVolume = useAppStore((s) => s.selectedVolume);
  const setSelectedVolume = useAppStore((s) => s.setSelectedVolume);
  const hiddenInstances = useAppStore((s) => s.hiddenInstances);
  const toggleInstanceVisibility = useAppStore((s) => s.toggleInstanceVisibility);
  const openContextMenu = useAppStore((s) => s.openContextMenu);
  const isSelected = selectedVolume === node.volume_name;
  const isHidden = hiddenInstances.has(node.instance_id);

  return (
    <div>
      <div
        onClick={() => setSelectedVolume(node.volume_name)}
        onContextMenu={(e) => {
          e.preventDefault();
          e.stopPropagation();
          const hidden = hiddenInstances.has(node.instance_id);
          openContextMenu(e.clientX, e.clientY, [
            { label: hidden ? 'Show' : 'Hide', action: () => toggleInstanceVisibility(node.instance_id) },
            { label: 'Change Material', action: () => { setSelectedVolume(node.volume_name); } },
          ]);
        }}
        style={{
          paddingLeft: depth * 12 + 4,
          paddingTop: 2,
          paddingBottom: 2,
          cursor: 'pointer',
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
        <span
          onClick={(e) => {
            e.stopPropagation();
            toggleInstanceVisibility(node.instance_id);
          }}
          style={{
            cursor: 'pointer',
            marginRight: 4,
            fontSize: 10,
            opacity: isHidden ? 0.4 : 0.8,
            flexShrink: 0,
          }}
          title={isHidden ? 'Show volume' : 'Hide volume'}
        >
          {isHidden ? '\u{1F441}\u{200D}\u{1F5E8}' : '\u{1F441}'}
        </span>
        <span style={{ overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {node.children.length > 0 ? '\u25B6 ' : '  '}
          {node.name}
        </span>
      </div>
      {node.children.map((child, i) => (
        <VolumeNode key={child.instance_id || `${child.volume_name}-${i}`} node={child} depth={depth + 1} />
      ))}
    </div>
  );
}
