import { useRef, useEffect } from 'react';
import * as THREE from 'three';
import { useThree, useFrame } from '@react-three/fiber';
import { useAppStore } from '../../store';
import { findBestSnap, allMeshCandidates } from './snapUtils';
import type { SnapPoint } from '../../store/types';

const _raycaster = new THREE.Raycaster();
const _mouse = new THREE.Vector2();

export function useMeasureRaycast() {
  const { camera, gl, scene } = useThree();
  const mouseRef = useRef({ x: 0, y: 0 });
  const lastSnapRef = useRef<SnapPoint | null>(null);
  const lastScreenRef = useRef({ x: 0, y: 0 });
  const lastHitMeshRef = useRef<THREE.Mesh | null>(null);
  const prevPendingRef = useRef<SnapPoint | null>(null);

  // Track mouse position without re-renders
  useEffect(() => {
    const canvas = gl.domElement;
    const onMove = (e: PointerEvent) => {
      const rect = canvas.getBoundingClientRect();
      mouseRef.current.x = e.clientX - rect.left;
      mouseRef.current.y = e.clientY - rect.top;
    };
    canvas.addEventListener('pointermove', onMove);
    return () => canvas.removeEventListener('pointermove', onMove);
  }, [gl]);

  useFrame(() => {
    const measureMode = useAppStore.getState().measureMode;
    if (!measureMode) {
      if (lastSnapRef.current) {
        lastSnapRef.current = null;
        useAppStore.getState().setHoverSnap(null);
        useAppStore.getState().setHoverCandidates([]);
      }
      return;
    }

    const canvas = gl.domElement;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    const mx = mouseRef.current.x;
    const my = mouseRef.current.y;

    // Convert to NDC
    _mouse.x = (mx / w) * 2 - 1;
    _mouse.y = -(my / h) * 2 + 1;

    _raycaster.setFromCamera(_mouse, camera);

    // Collect meshes, filter out measurement markers
    const meshes: THREE.Object3D[] = [];
    scene.traverse((obj) => {
      if ((obj as THREE.Mesh).isMesh && !obj.userData.isMeasureTool) {
        meshes.push(obj);
      }
    });

    const hits = _raycaster.intersectObjects(meshes, false);

    // Track the first hit mesh for surface candidate computation
    if (hits.length > 0) {
      lastHitMeshRef.current = hits[0].object as THREE.Mesh;
    }

    // Detect pendingPoint transitions: null→non-null or non-null→null
    const currentPending = useAppStore.getState().pendingPoint;
    const prevPending = prevPendingRef.current;
    if (!prevPending && currentPending) {
      // Just placed point A — compute all snap candidates on the clicked mesh
      if (lastHitMeshRef.current) {
        const surface = allMeshCandidates(lastHitMeshRef.current);
        useAppStore.getState().setSurfaceCandidates(surface);
      }
    } else if (prevPending && !currentPending) {
      // Point B placed or cancelled — clear surface candidates
      useAppStore.getState().setSurfaceCandidates([]);
    }
    prevPendingRef.current = currentPending;

    if (hits.length === 0) {
      if (lastSnapRef.current) {
        lastSnapRef.current = null;
        useAppStore.getState().setHoverSnap(null);
        useAppStore.getState().setHoverCandidates([]);
      }
      return;
    }

    const result = findBestSnap(hits, mx, my, camera, w, h);
    if (!result) {
      if (lastSnapRef.current) {
        lastSnapRef.current = null;
        useAppStore.getState().setHoverSnap(null);
        useAppStore.getState().setHoverCandidates([]);
      }
      return;
    }

    const snap = result.best;

    // Hysteresis: only switch if new candidate is >3px closer or different type
    if (lastSnapRef.current) {
      const prev = lastSnapRef.current;
      if (prev.type === snap.type) {
        const dx = prev.position[0] - snap.position[0];
        const dy = prev.position[1] - snap.position[1];
        const dz = prev.position[2] - snap.position[2];
        const worldDist = Math.sqrt(dx * dx + dy * dy + dz * dz);
        if (worldDist < 0.01) return; // same point, skip update
      }
    }

    lastSnapRef.current = snap;
    useAppStore.getState().setHoverSnap(snap);
    useAppStore.getState().setHoverCandidates(result.candidates);
  });
}
