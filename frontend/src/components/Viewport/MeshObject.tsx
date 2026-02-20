import { useEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import type { MeshData } from '../../store/types';
import { useAppStore } from '../../store';

interface Props {
  meshData: MeshData;
  color: string;
  selected: boolean;
  name: string;
}

export default function MeshObject({ meshData, color, selected, name }: Props) {
  const meshRef = useRef<THREE.Mesh>(null);

  const geometry = useMemo(() => {
    const geo = new THREE.BufferGeometry();
    geo.setAttribute(
      'position',
      new THREE.Float32BufferAttribute(meshData.positions, 3),
    );
    geo.setAttribute(
      'normal',
      new THREE.Float32BufferAttribute(meshData.normals, 3),
    );
    geo.setIndex(new THREE.Uint32BufferAttribute(meshData.indices, 1));
    return geo;
  }, [meshData]);

  useEffect(() => {
    return () => { geometry.dispose(); };
  }, [geometry]);

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
