import { useEffect, useLayoutEffect, useRef, useMemo } from 'react';
import * as THREE from 'three';
import { Canvas, useThree, addEffect } from '@react-three/fiber';
import { OrbitControls, GizmoHelper, GizmoViewport } from '@react-three/drei';
import Scene from './Scene';
import MeasureInteraction from './MeasureInteraction';
import MeasureOverlay from './MeasureOverlay';
import { useAppStore } from '../../store';

/**
 * Fits camera and controls to the scene meshes once per loaded document.
 * `meshes` is replaced only by a load, so edits, hiding and selection keep the view.
 */
function AutoFitCamera() {
  const { camera, scene, invalidate } = useThree();
  const meshes = useAppStore((s) => s.meshes);
  const controlsRef = useRef<{ target: THREE.Vector3; update: () => void } | null>(null);

  useEffect(() => {
    if (Object.keys(meshes).length === 0) return;

    // Mesh objects mount over the next frames (each acquires its geometry in an
    // effect), so retry each frame until they are in the scene.
    let frames = 0;
    let id = requestAnimationFrame(function fit() {
      scene.updateMatrixWorld();
      const box = new THREE.Box3();
      scene.traverse((obj) => {
        if ((obj as THREE.Mesh).isMesh && !obj.userData.isMeasureTool) {
          box.expandByObject(obj);
        }
      });
      if (box.isEmpty()) {
        if (++frames < 60) id = requestAnimationFrame(fit);
        return;
      }

      const center = new THREE.Vector3();
      const size = new THREE.Vector3();
      box.getCenter(center);
      box.getSize(size);

      const maxDim = Math.max(size.x, size.y, size.z);
      // Bail on a degenerate/non-finite box; otherwise camera near/far/position become NaN.
      if (
        !Number.isFinite(maxDim) || maxDim <= 0 ||
        !Number.isFinite(center.x) || !Number.isFinite(center.y) || !Number.isFinite(center.z)
      ) {
        return;
      }
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
      invalidate();
    });

    return () => cancelAnimationFrame(id);
  }, [meshes, camera, scene, invalidate]);

  const measureMode = useAppStore((s) => s.measureMode);

  // In measure mode, free left-click for measurement: left=none, middle=dolly, right=rotate
  // Always pass explicit config — OrbitControls doesn't reset when prop goes from object to undefined
  const mouseButtons = measureMode
    ? { LEFT: undefined as unknown as THREE.MOUSE, MIDDLE: THREE.MOUSE.DOLLY, RIGHT: THREE.MOUSE.ROTATE }
    : { LEFT: THREE.MOUSE.ROTATE, MIDDLE: THREE.MOUSE.DOLLY, RIGHT: THREE.MOUSE.PAN };

  return <OrbitControls makeDefault ref={controlsRef as never} mouseButtons={mouseButtons} />;
}

/** The frames frameloop="demand" needs that R3F does not request itself. */
function DemandFrames() {
  const clock = useThree((s) => s.clock);
  const invalidate = useThree((s) => s.invalidate);
  // R3F requests a frame when it adds or updates an object but not when it removes one (hide, clear).
  // Request it after this root commits the store change: a store subscription fires before the
  // commit, so the frame could draw the removed mesh again (seen with the 3D menu's Hide).
  useAppStore();
  useLayoutEffect(() => {
    invalidate();
  });
  // A frame after a pause gets the whole pause as its delta, and GizmoHelper's tween would jump.
  useEffect(() => addEffect(() => { clock.oldTime = Math.max(clock.oldTime, performance.now() - 50); }), [clock]);
  return null;
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
    // Round up to a nice number. Guard maxCoord==0 (all-zero meshes): log10(0) is
    // -Infinity → Math.pow(10, -Infinity) === 0, which would hide the grid/axes.
    const grid = Math.pow(10, Math.ceil(Math.log10(maxCoord * 2)));
    const safeGrid = Number.isFinite(grid) && grid > 0 ? grid : 2000;
    return { grid: safeGrid, axes: safeGrid * 0.1 };
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
    <div style={{ width: '100%', height: '100%', background: '#0d1117', position: 'relative' }} onContextMenu={(e) => e.preventDefault()}>
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
      {/* Renders only on change: R3F commits, store changes, OrbitControls/Gizmo moves, AutoFit. */}
      <Canvas
        flat
        frameloop="demand"
        camera={{ position: [500, 500, 500], fov: 50, near: 0.1, far: 100000 }}
        gl={{ antialias: true }}
      >
        <color attach="background" args={['#0d1117']} />
        <ambientLight intensity={1.0} />
        <hemisphereLight args={['#b1e1ff', '#666680', 0.8]} />
        <directionalLight position={[500, 500, 500]} intensity={1.8} />
        <directionalLight position={[-300, -200, -400]} intensity={1.0} />
        <directionalLight position={[0, -500, 300]} intensity={0.7} />
        <directionalLight position={[0, 500, -500]} intensity={0.5} />
        <directionalLight position={[-500, 0, 0]} intensity={0.5} />
        <directionalLight position={[500, 0, -500]} intensity={0.4} />
        {sceneGraph && <Scene node={sceneGraph} />}
        <DynamicGrid />
        <AutoFitCamera />
        <DemandFrames />
        <MeasureInteraction />
        <MeasureOverlay />
        <GizmoHelper alignment="bottom-right" margin={[60, 60]}>
          <GizmoViewport />
        </GizmoHelper>
      </Canvas>
    </div>
  );
}
