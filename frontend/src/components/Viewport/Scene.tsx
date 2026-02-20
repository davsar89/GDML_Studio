import { useMemo } from 'react';
import * as THREE from 'three';
import MeshObject from './MeshObject';
import { useAppStore } from '../../store';
import type { SceneNode } from '../../store/types';

function materialColor(materialName: string, auxColor: string | null): string {
  if (auxColor && auxColor.length >= 6) {
    return `#${auxColor}`;
  }
  // Hash material name to HSL color
  let hash = 0;
  for (let i = 0; i < materialName.length; i++) {
    hash = materialName.charCodeAt(i) + ((hash << 5) - hash);
  }
  const h = ((hash % 360) + 360) % 360;
  return `hsl(${h}, 55%, 55%)`;
}

export default function Scene({ node }: { node: SceneNode }) {
  return <SceneNodeGroup node={node} />;
}

function SceneNodeGroup({ node }: { node: SceneNode }) {
  const meshes = useAppStore((s) => s.meshes);
  const selectedVolume = useAppStore((s) => s.selectedVolume);
  const meshData = meshes[node.solid_name];
  const isSelected = selectedVolume === node.volume_name;

  // GDML applies extrinsic rotations Rx·Ry·Rz → matrix Rz·Ry·Rx → Three.js Euler 'XYZ'
  const euler = useMemo(() => {
    const [rx, ry, rz] = node.rotation;
    return new THREE.Euler(rx, ry, rz, 'XYZ');
  }, [node.rotation]);

  const color = materialColor(node.material_name, node.color);

  // Skip world volume mesh (usually huge bounding sphere/box) but render children
  const skipMesh = node.is_world || !meshData;

  return (
    <group position={node.position as [number, number, number]} rotation={euler}>
      {!skipMesh && meshData && (
        <MeshObject
          meshData={meshData}
          color={color}
          selected={isSelected}
          name={node.volume_name}
        />
      )}
      {node.children.map((child, i) => (
        <SceneNodeGroup key={`${child.volume_name}-${i}`} node={child} />
      ))}
    </group>
  );
}
