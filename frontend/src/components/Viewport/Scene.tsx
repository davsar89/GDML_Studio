import { useMemo } from 'react';
import * as THREE from 'three';
import MeshObject from './MeshObject';
import { useAppStore } from '../../store';
import type { SceneNode } from '../../store/types';

// Bright fallback palette for materials without density info
const PALETTE = [
  '#64B5F6', // blue
  '#81C784', // green
  '#FFB74D', // orange
  '#CE93D8', // purple
  '#FF8A65', // deep orange
  '#4DD0E1', // cyan
  '#AED581', // light green
  '#F06292', // pink
  '#FFD54F', // amber
  '#9FA8DA', // indigo
  '#BCAAA4', // brown
  '#B0BEC5', // blue grey
  '#EF9A9A', // red
  '#4FC3F7', // light blue
  '#E6EE9C', // lime
  '#80CBC4', // teal
];

/** Map material density (g/cm³) to an HSL color.
 *  Light materials → cool bright colors, heavy → warm dark colors. */
function densityToColor(density: number): string {
  // Clamp density to a useful range
  const d = Math.max(0.001, Math.min(density, 22));
  // Use log scale: ln(0.001)≈-6.9, ln(22)≈3.1 → normalize to 0..1
  const t = (Math.log(d) - (-6.9)) / (3.1 - (-6.9)); // 0 = ultra-light, 1 = ultra-heavy

  // Hue: 200 (blue) → 45 (gold) → 0 (red) → 280 (purple) for very heavy
  let hue: number;
  if (t < 0.5) {
    hue = 200 - t * 2 * 155; // 200 → 45
  } else if (t < 0.8) {
    hue = 45 - ((t - 0.5) / 0.3) * 45; // 45 → 0
  } else {
    hue = 360 - ((t - 0.8) / 0.2) * 80; // 360 → 280
  }

  // Saturation: 60% to 75% — vivid colors
  const sat = 60 + t * 15;
  // Lightness: 75% for lightest down to 50% for heaviest — always bright enough
  const light = 75 - t * 25;

  return `hsl(${Math.round(hue)}, ${Math.round(sat)}%, ${Math.round(light)}%)`;
}

function materialColor(materialName: string, auxColor: string | null, density: number | null): string {
  if (auxColor && auxColor.length >= 6) {
    // Skip near-black aux colors (common in GDML files exported from other tools)
    const r = parseInt(auxColor.substring(0, 2), 16);
    const g = parseInt(auxColor.substring(2, 4), 16);
    const b = parseInt(auxColor.substring(4, 6), 16);
    const luminance = 0.299 * r + 0.587 * g + 0.114 * b;
    if (luminance > 20) {
      return `#${auxColor}`;
    }
  }
  if (density != null && density > 0) {
    return densityToColor(density);
  }
  // Fallback: hash material name into bright palette
  let hash = 0;
  for (let i = 0; i < materialName.length; i++) {
    hash = materialName.charCodeAt(i) + ((hash << 5) - hash);
  }
  const idx = ((hash % PALETTE.length) + PALETTE.length) % PALETTE.length;
  return PALETTE[idx];
}

function getMaxDepth(node: SceneNode, d = 0): number {
  if (node.children.length === 0) return d;
  return Math.max(...node.children.map(c => getMaxDepth(c, d + 1)));
}

export default function Scene({ node }: { node: SceneNode }) {
  const maxDepth = useMemo(() => getMaxDepth(node), [node]);
  return <SceneNodeGroup node={node} depth={0} maxDepth={maxDepth} />;
}

function SceneNodeGroup({ node, depth, maxDepth }: { node: SceneNode; depth: number; maxDepth: number }) {
  const meshes = useAppStore((s) => s.meshes);
  const selectedVolume = useAppStore((s) => s.selectedVolume);
  const hiddenVolumes = useAppStore((s) => s.hiddenVolumes);
  const meshData = meshes[node.solid_name];
  const isSelected = selectedVolume === node.volume_name;
  const isHidden = hiddenVolumes.has(node.volume_name);

  // GDML applies extrinsic rotations Rx·Ry·Rz → matrix Rz·Ry·Rx → Three.js Euler 'XYZ'
  const euler = useMemo(() => {
    const [rx, ry, rz] = node.rotation;
    return new THREE.Euler(rx, ry, rz, 'XYZ');
  }, [node.rotation]);

  const color = materialColor(node.material_name, node.color, node.density);

  // Skip world volume mesh (usually huge bounding sphere/box) but render children
  const skipMesh = node.is_world || !meshData || isHidden;

  return (
    <group position={node.position as [number, number, number]} rotation={euler}>
      {!skipMesh && meshData && (
        <MeshObject
          meshData={meshData}
          color={color}
          selected={isSelected}
          name={node.volume_name}
          solidName={node.solid_name}
          depth={depth}
          maxDepth={maxDepth}
        />
      )}
      {node.children.map((child, i) => (
        <SceneNodeGroup key={`${child.volume_name}-${i}`} node={child} depth={depth + 1} maxDepth={maxDepth} />
      ))}
    </group>
  );
}
