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
  const isSelected = selectedVolume === node.volume_name;

  return (
    <div>
      <div
        onClick={() => setSelectedVolume(node.volume_name)}
        style={{
          paddingLeft: depth * 12 + 4,
          paddingTop: 2,
          paddingBottom: 2,
          cursor: 'pointer',
          fontSize: 11,
          fontFamily: 'monospace',
          background: isSelected ? '#0f3460' : 'transparent',
          color: isSelected ? '#e94560' : '#b0b8c0',
          borderRadius: 2,
          whiteSpace: 'nowrap',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
        }}
        title={`${node.volume_name} [${node.solid_name}] (${node.material_name})`}
      >
        {node.children.length > 0 ? '\u25B6 ' : '  '}
        {node.name}
      </div>
      {node.children.map((child, i) => (
        <VolumeNode key={`${child.volume_name}-${i}`} node={child} depth={depth + 1} />
      ))}
    </div>
  );
}
