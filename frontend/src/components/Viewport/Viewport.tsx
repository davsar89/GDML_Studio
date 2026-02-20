import { useEffect, useRef, useMemo } from 'react';
import * as THREE from 'three';
import { Canvas, useThree } from '@react-three/fiber';
import { OrbitControls, GizmoHelper, GizmoViewport } from '@react-three/drei';
import Scene from './Scene';
import { useAppStore } from '../../store';

/** Computes the bounding sphere of all scene meshes and adjusts camera + controls. */
function AutoFitCamera({ sceneGraph }: { sceneGraph: unknown }) {
  const { camera, scene } = useThree();
  const controlsRef = useRef<{ target: THREE.Vector3; update: () => void } | null>(null);

  useEffect(() => {
    if (!sceneGraph) return;

    // Wait a frame for meshes to be added to the scene
    const id = requestAnimationFrame(() => {
      const box = new THREE.Box3();
      scene.traverse((obj) => {
        if ((obj as THREE.Mesh).isMesh) {
          box.expandByObject(obj);
        }
      });
      if (box.isEmpty()) return;

      const center = new THREE.Vector3();
      const size = new THREE.Vector3();
      box.getCenter(center);
      box.getSize(size);

      const maxDim = Math.max(size.x, size.y, size.z);
      const fov = (camera as THREE.PerspectiveCamera).fov * (Math.PI / 180);
      const dist = maxDim / (2 * Math.tan(fov / 2)) * 1.5;

      camera.position.set(center.x + dist * 0.6, center.y + dist * 0.5, center.z + dist * 0.7);
      (camera as THREE.PerspectiveCamera).near = maxDim * 0.001;
      (camera as THREE.PerspectiveCamera).far = maxDim * 20;
      camera.updateProjectionMatrix();
      camera.lookAt(center);

      // Update OrbitControls target
      const controls = controlsRef.current;
      if (controls) {
        controls.target.copy(center);
        controls.update();
      }
    });

    return () => cancelAnimationFrame(id);
  }, [sceneGraph, camera, scene]);

  return <OrbitControls makeDefault ref={controlsRef as never} />;
}

/** Dynamically sized grid + axes based on loaded meshes. */
function DynamicGrid() {
  const meshes = useAppStore((s) => s.meshes);

  const gridSize = useMemo(() => {
    const keys = Object.keys(meshes);
    if (keys.length === 0) return { grid: 2000, axes: 200 };

    let maxCoord = 0;
    for (const key of keys) {
      const m = meshes[key];
      for (let i = 0; i < m.positions.length; i++) {
        maxCoord = Math.max(maxCoord, Math.abs(m.positions[i]));
      }
    }
    // Round up to a nice number
    const grid = Math.pow(10, Math.ceil(Math.log10(maxCoord * 2)));
    return { grid, axes: grid * 0.1 };
  }, [meshes]);

  return (
    <>
      <gridHelper args={[gridSize.grid, 40, '#333344', '#222233']} />
      <axesHelper args={[gridSize.axes]} />
    </>
  );
}

export default function Viewport() {
  const sceneGraph = useAppStore((s) => s.sceneGraph);
  const loading = useAppStore((s) => s.loading);

  return (
    <div style={{ width: '100%', height: '100%', background: '#0d1117', position: 'relative' }}>
      {loading && (
        <div style={{
          position: 'absolute',
          inset: 0,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          zIndex: 10,
          color: '#8899aa',
          fontSize: 18,
          pointerEvents: 'none',
        }}>
          Loading...
        </div>
      )}
      <Canvas
        camera={{ position: [500, 500, 500], fov: 50, near: 0.1, far: 100000 }}
        gl={{ antialias: true }}
      >
        <color attach="background" args={['#0d1117']} />
        <ambientLight intensity={0.4} />
        <directionalLight position={[500, 500, 500]} intensity={0.8} />
        <directionalLight position={[-300, -200, -400]} intensity={0.3} />
        {sceneGraph && <Scene node={sceneGraph} />}
        <DynamicGrid />
        <AutoFitCamera sceneGraph={sceneGraph} />
        <GizmoHelper alignment="bottom-right" margin={[60, 60]}>
          <GizmoViewport />
        </GizmoHelper>
      </Canvas>
    </div>
  );
}
