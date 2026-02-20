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
}

export default function MeshObject({ meshData, color, selected, name, solidName }: Props) {
  const meshRef = useRef<THREE.Mesh>(null);
  const geometry = getOrCreateGeometry(solidName, meshData);

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
        emissive={selected ? '#ff2060' : '#000000'}
        emissiveIntensity={selected ? 0.3 : 0}
        metalness={0.1}
        roughness={0.6}
      />
    </mesh>
  );
}
