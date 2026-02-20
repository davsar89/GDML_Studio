import { useEffect, useRef } from 'react';
import * as THREE from 'three';
import type { MeshData } from '../../store/types';
import { useAppStore } from '../../store';
import { getOrCreateGeometry, releaseGeometry } from './geometryCache';

interface Props {
  meshData: MeshData;
  color: string;
  selected: boolean;
  name: string;
  solidName: string;
  depth: number;
  maxDepth: number;
}

export default function MeshObject({ meshData, color, selected, name, solidName, depth, maxDepth }: Props) {
  const meshRef = useRef<THREE.Mesh>(null);
  const meshOpacity = useAppStore((s) => s.meshOpacity);
  const geometry = getOrCreateGeometry(solidName, meshData);

  // Depth-weighted opacity: power curve so outer fades first, inner resists.
  // At 0% everything invisible, at 100% everything opaque.
  const depthFactor = maxDepth > 0 ? depth / maxDepth : 0;
  const effectiveOpacity = Math.pow(meshOpacity, 3 - 2 * depthFactor);

  useEffect(() => {
    return () => { releaseGeometry(solidName); };
  }, [solidName]);

  const handleClick = (e: THREE.Event) => {
    // @ts-expect-error: ThreeEvent stopPropagation
    e.stopPropagation?.();
    useAppStore.getState().setSelectedVolume(name);
  };

  return (
    <mesh ref={meshRef} geometry={geometry} onClick={handleClick}>
      <meshStandardMaterial
        color={selected ? '#ff6090' : color}
        side={THREE.DoubleSide}
        emissive={selected ? '#ff2060' : color}
        emissiveIntensity={selected ? 0.3 : 0.12}
        metalness={0.15}
        roughness={0.45}
        transparent={effectiveOpacity < 1}
        opacity={effectiveOpacity}
        depthWrite={effectiveOpacity >= 1}
      />
    </mesh>
  );
}
