import { useEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import { useFrame, useThree } from '@react-three/fiber';
import { Html } from '@react-three/drei';
import { useAppStore } from '../../store';
import { formatDistance } from './snapUtils';

const SNAP_COLORS: Record<string, number> = {
  vertex: 0x22c55e, // green
  edge: 0xf59e0b,   // amber
  face: 0x06b6d4,   // cyan
};

const LINE_COLOR = 0xe94560;

function noRaycast() {}

/** Small endpoint dot: filled sphere, constant screen size, always-on-top */
function SnapDot({ position, type }: { position: [number, number, number]; type: string }) {
  const groupRef = useRef<THREE.Group>(null);
  const { camera } = useThree();
  const color = SNAP_COLORS[type] ?? SNAP_COLORS.face;

  useFrame(() => {
    if (!groupRef.current) return;
    const dist = camera.position.distanceTo(groupRef.current.position);
    const s = dist * 0.005;
    groupRef.current.scale.setScalar(s);
  });

  return (
    <group ref={groupRef} position={position} renderOrder={999}>
      <mesh raycast={noRaycast} userData={{ isMeasureTool: true }}>
        <sphereGeometry args={[1, 16, 16]} />
        <meshBasicMaterial color={color} depthTest={false} depthWrite={false} />
      </mesh>
    </group>
  );
}

/** Renders all snap candidates as small black dots, with the active one highlighted white.
 *  After placing point A, shows ALL surface candidates on the clicked mesh instead of just hover candidates. */
function SnapCandidates() {
  const hoverCandidates = useAppStore((s) => s.hoverCandidates);
  const surfaceCandidates = useAppStore((s) => s.surfaceCandidates);
  const pendingPoint = useAppStore((s) => s.pendingPoint);
  const hoverSnap = useAppStore((s) => s.hoverSnap);
  const measureMode = useAppStore((s) => s.measureMode);
  const groupRef = useRef<THREE.Group>(null);
  const { camera } = useThree();

  // Show surface candidates when point A is placed, otherwise hover candidates
  const candidates = pendingPoint && surfaceCandidates.length > 0 ? surfaceCandidates : hoverCandidates;

  useFrame(() => {
    if (!groupRef.current) return;
    // Scale all candidate dots based on camera distance to group center
    groupRef.current.children.forEach((child) => {
      const dist = camera.position.distanceTo(child.position);
      const isActive = child.userData.isActiveSnap;
      const s = isActive ? dist * 0.004 : dist * 0.003;
      child.scale.setScalar(s);
    });
  });

  if (!measureMode || candidates.length === 0) return null;

  return (
    <group ref={groupRef}>
      {candidates.map((c, i) => {
        const isActive =
          hoverSnap != null &&
          Math.abs(c.position[0] - hoverSnap.position[0]) < 0.001 &&
          Math.abs(c.position[1] - hoverSnap.position[1]) < 0.001 &&
          Math.abs(c.position[2] - hoverSnap.position[2]) < 0.001;

        return (
          <mesh
            key={i}
            position={c.position}
            renderOrder={999}
            raycast={noRaycast}
            userData={{ isMeasureTool: true, isActiveSnap: isActive }}
          >
            <sphereGeometry args={[1, 12, 12]} />
            <meshBasicMaterial
              color={isActive ? 0xffffff : 0x000000}
              depthTest={false}
              depthWrite={false}
            />
          </mesh>
        );
      })}
    </group>
  );
}

/** Dashed line using native THREE.Line + LineDashedMaterial */
function DashedLine({ start, end }: { start: [number, number, number]; end: [number, number, number] }) {
  // Destructure before the memo so the closure reads six scalars rather than the
  // two tuples. Callers pass fresh arrays each render, so depending on the tuples
  // would rebuild the geometry every frame; depending on `start[0]`-style member
  // expressions instead makes React Compiler infer `start`/`end` and bail out of
  // optimizing this component entirely.
  const [sx, sy, sz] = start;
  const [ex, ey, ez] = end;

  const geometry = useMemo(() => {
    const geom = new THREE.BufferGeometry();
    geom.setAttribute(
      'position',
      new THREE.Float32BufferAttribute([sx, sy, sz, ex, ey, ez], 3),
    );
    return geom;
  }, [sx, sy, sz, ex, ey, ez]);

  const material = useMemo(
    () => new THREE.LineDashedMaterial({ color: LINE_COLOR, dashSize: 4, gapSize: 3, depthTest: false, depthWrite: false }),
    [],
  );

  // Memoize the Line itself (avoids allocating a new THREE.Line every render) and
  // compute dash distances once per geometry/material change.
  const line = useMemo(() => {
    const ln = new THREE.Line(geometry, material);
    ln.computeLineDistances();
    return ln;
  }, [geometry, material]);

  // Dispose GPU resources when geometry/material are replaced or on unmount.
  useEffect(() => () => geometry.dispose(), [geometry]);
  useEffect(() => () => material.dispose(), [material]);

  return (
    <primitive
      object={line}
      raycast={noRaycast}
      userData={{ isMeasureTool: true }}
      renderOrder={998}
    />
  );
}

/** A measured segment: dashed line from a to b, with its length at the midpoint. */
function Segment({ a, b }: { a: [number, number, number]; b: [number, number, number] }) {
  return (
    <>
      <DashedLine start={a} end={b} />
      <Html position={a.map((v, i) => (v + b[i]) / 2) as [number, number, number]} center style={{ pointerEvents: 'none' }}>
        <div
          style={{
            background: 'rgba(22,33,62,0.92)',
            color: '#e0e0e0',
            padding: '3px 10px',
            borderRadius: 10,
            fontSize: 12,
            fontFamily: 'monospace',
            whiteSpace: 'nowrap',
            border: '1px solid rgba(233,69,96,0.6)',
            userSelect: 'none',
          }}
        >
          {formatDistance(Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2]))}
        </div>
      </Html>
    </>
  );
}

export default function MeasureOverlay() {
  const measureMode = useAppStore((s) => s.measureMode);
  const measurements = useAppStore((s) => s.measurements);
  const pendingPoint = useAppStore((s) => s.pendingPoint);
  const hoverSnap = useAppStore((s) => s.hoverSnap);

  return (
    <group>
      {measurements.map((m) => (
        <group key={m.id}>
          <SnapDot position={m.pointA.position} type={m.pointA.type} />
          <SnapDot position={m.pointB.position} type={m.pointB.type} />
          <Segment a={m.pointA.position} b={m.pointB.position} />
        </group>
      ))}

      {/* Snap candidate indicators */}
      <SnapCandidates />

      {/* Pending point and the live preview to the hovered snap */}
      {pendingPoint && <SnapDot position={pendingPoint.position} type={pendingPoint.type} />}
      {pendingPoint && hoverSnap && measureMode && <Segment a={pendingPoint.position} b={hoverSnap.position} />}
    </group>
  );
}
