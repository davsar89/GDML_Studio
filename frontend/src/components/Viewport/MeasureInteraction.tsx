import { useEffect } from 'react';
import * as THREE from 'three';
import { useThree } from '@react-three/fiber';
import { useAppStore } from '../../store';
import { findBestSnap, allMeshCandidates } from './snapUtils';

/** Measure mode input. Raycasts only on pointer events, never per frame. */
export default function MeasureInteraction() {
  const { camera, gl, scene } = useThree();

  useEffect(() => {
    const canvas = gl.domElement;
    const raycaster = new THREE.Raycaster();
    let frame = 0;

    /** The snap under the pointer and the mesh it lies on, or null over empty space. */
    const snapAt = (e: PointerEvent) => {
      const rect = canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      raycaster.setFromCamera(new THREE.Vector2((x / rect.width) * 2 - 1, -(y / rect.height) * 2 + 1), camera);
      const meshes: THREE.Object3D[] = [];
      scene.traverse((obj) => {
        if ((obj as THREE.Mesh).isMesh && !obj.userData.isMeasureTool) meshes.push(obj);
      });
      const hits = raycaster.intersectObjects(meshes, false);
      const result = findBestSnap(hits, x, y, camera, rect.width, rect.height);
      return result && { ...result, mesh: hits[0].object as THREE.Mesh };
    };

    const onPointerMove = (e: PointerEvent) => {
      if (!useAppStore.getState().measureMode) return;
      // At most one raycast per frame, for the latest position.
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const state = useAppStore.getState();
        if (!state.measureMode) return;
        const result = snapAt(e);
        const prev = state.hoverSnap;
        const snap = result?.best ?? null;
        const same = prev && snap
          ? prev.type === snap.type && Math.hypot(...prev.position.map((v, i) => v - snap.position[i])) < 0.01
          : prev === snap;
        if (same) return;
        state.setHoverSnap(snap);
        state.setHoverCandidates(result?.candidates ?? []);
      });
    };

    // Snap at the click itself: a hover from an earlier frame may be stale or absent.
    const onPointerDown = (e: PointerEvent) => {
      const state = useAppStore.getState();
      if (e.button !== 0 || !state.measureMode) return;
      const result = snapAt(e);
      if (!result) return;
      const placingA = !state.pendingPoint;
      state.placeMeasurePoint(result.best);
      if (placingA) state.setSurfaceCandidates(allMeshCandidates(result.mesh));
    };

    const onKeyDown = (e: KeyboardEvent) => {
      const state = useAppStore.getState();
      if (e.key !== 'Escape' || !state.measureMode) return;
      // Escape in a form field belongs to that field.
      if (e.target instanceof Element && e.target.closest('input, textarea, select')) return;
      e.preventDefault();
      state.cancelMeasure();
    };

    canvas.addEventListener('pointermove', onPointerMove);
    canvas.addEventListener('pointerdown', onPointerDown);
    window.addEventListener('keydown', onKeyDown);
    return () => {
      cancelAnimationFrame(frame);
      canvas.removeEventListener('pointermove', onPointerMove);
      canvas.removeEventListener('pointerdown', onPointerDown);
      window.removeEventListener('keydown', onKeyDown);
    };
  }, [camera, gl, scene]);

  // Cursor style
  useEffect(() => {
    const canvas = gl.domElement;
    const applyCursor = (measureMode: boolean) => {
      // React Compiler treats values returned from hooks as immutable, and the
      // canvas reaches us via useThree(). Styling it imperatively is the only
      // way to drive the cursor on an R3F canvas, so the rule is opted out of
      // here rather than worked around.
      // eslint-disable-next-line react-hooks/immutability
      canvas.style.cursor = measureMode ? 'crosshair' : '';
    };

    const unsubscribe = useAppStore.subscribe((state) => applyCursor(state.measureMode));
    applyCursor(useAppStore.getState().measureMode);
    return () => {
      unsubscribe();
      applyCursor(false);
    };
  }, [gl]);

  return null;
}
