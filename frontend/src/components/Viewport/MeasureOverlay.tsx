import { useEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import { useFrame, useThree } from '@react-three/fiber';
import { Html } from '@react-three/drei';
import { useAppStore } from '../../store';
import { formatDistance } from './snapUtils';
import type { SnapPoint, Measurement } from '../../store/types';

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
function DashedLine({ start, end, color = LINE_COLOR, depthTest = false }: {
  start: [number, number, number];
  end: [number, number, number];
  color?: number;
  depthTest?: boolean;
}) {
  const lineRef = useRef<THREE.Line>(null);

  const geometry = useMemo(() => {
    const geom = new THREE.BufferGeometry();
    geom.setAttribute(
      'position',
      new THREE.Float32BufferAttribute([...start, ...end], 3),
    );
    return geom;
  }, [start[0], start[1], start[2], end[0], end[1], end[2]]);

  // computeLineDistances is required for dashed material
  useEffect(() => {
    if (lineRef.current) {
      lineRef.current.computeLineDistances();
    }
  }, [geometry]);

  const material = useMemo(
    () =>
      new THREE.LineDashedMaterial({
        color,
        dashSize: 4,
        gapSize: 3,
        depthTest,
        depthWrite: false,
      }),
    [color, depthTest],
  );

  return (
    <primitive
      ref={lineRef}
      object={new THREE.Line(geometry, material)}
      raycast={noRaycast}
      userData={{ isMeasureTool: true }}
      renderOrder={998}
    />
  );
}

/** Distance label rendered at a world position */
function DistanceLabel({ position, distance }: { position: [number, number, number]; distance: number }) {
  return (
    <Html position={position} center style={{ pointerEvents: 'none' }}>
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
        {formatDistance(distance)}
      </div>
    </Html>
  );
}

/** A completed measurement: two dots, a line, and a label */
function CompletedMeasurement({ m }: { m: Measurement }) {
  const midpoint: [number, number, number] = [
    (m.pointA.position[0] + m.pointB.position[0]) / 2,
    (m.pointA.position[1] + m.pointB.position[1]) / 2,
    (m.pointA.position[2] + m.pointB.position[2]) / 2,
  ];

  return (
    <group userData={{ isMeasureTool: true }}>
      <SnapDot position={m.pointA.position} type={m.pointA.type} />
      <SnapDot position={m.pointB.position} type={m.pointB.type} />
      <DashedLine start={m.pointA.position} end={m.pointB.position} />
      <DistanceLabel position={midpoint} distance={m.distance} />
    </group>
  );
}

/** Live preview line from pending point to hover snap */
function PendingPreview({ pending, hover }: { pending: SnapPoint; hover: SnapPoint }) {
  const dx = hover.position[0] - pending.position[0];
  const dy = hover.position[1] - pending.position[1];
  const dz = hover.position[2] - pending.position[2];
  const dist = Math.sqrt(dx * dx + dy * dy + dz * dz);

  const midpoint: [number, number, number] = [
    (pending.position[0] + hover.position[0]) / 2,
    (pending.position[1] + hover.position[1]) / 2,
    (pending.position[2] + hover.position[2]) / 2,
  ];

  return (
    <group userData={{ isMeasureTool: true }}>
      <DashedLine start={pending.position} end={hover.position} />
      <DistanceLabel position={midpoint} distance={dist} />
    </group>
  );
}

export default function MeasureOverlay() {
  const measureMode = useAppStore((s) => s.measureMode);
  const measurements = useAppStore((s) => s.measurements);
  const pendingPoint = useAppStore((s) => s.pendingPoint);
  const hoverSnap = useAppStore((s) => s.hoverSnap);

  return (
    <group>
      {/* Completed measurements (always visible) */}
      {measurements.map((m) => (
        <CompletedMeasurement key={m.id} m={m} />
      ))}

      {/* Snap candidate indicators */}
      <SnapCandidates />

      {/* Pending point dot */}
      {pendingPoint && (
        <SnapDot position={pendingPoint.position} type={pendingPoint.type} />
      )}

      {/* Preview line from pending to hover */}
      {pendingPoint && hoverSnap && measureMode && (
        <PendingPreview pending={pendingPoint} hover={hoverSnap} />
      )}
    </group>
  );
}
