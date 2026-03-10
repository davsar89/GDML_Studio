import * as THREE from 'three';
import type { SnapPoint } from '../../store/types';

const _v = new THREE.Vector3();
const _proj = new THREE.Vector3();

interface SnapCandidate {
  position: THREE.Vector3;
  type: 'vertex' | 'edge' | 'face';
}

/**
 * Project a world-space point to screen-space pixels.
 */
function toScreen(pos: THREE.Vector3, camera: THREE.Camera, w: number, h: number): { x: number; y: number } {
  _proj.copy(pos).project(camera);
  return {
    x: (_proj.x * 0.5 + 0.5) * w,
    y: (-_proj.y * 0.5 + 0.5) * h,
  };
}

/**
 * Extract 7 snap candidates (3 vertices, 3 edge midpoints, 1 centroid) from a face.
 */
function faceCandidates(intersection: THREE.Intersection): SnapCandidate[] {
  const mesh = intersection.object as THREE.Mesh;
  const geometry = mesh.geometry as THREE.BufferGeometry;
  const faceIndex = intersection.faceIndex;

  if (faceIndex == null) {
    const p = intersection.point;
    return [{ position: new THREE.Vector3(p.x, p.y, p.z), type: 'face' }];
  }

  const posAttr = geometry.getAttribute('position') as THREE.BufferAttribute;
  const index = geometry.getIndex();

  let ia: number, ib: number, ic: number;
  if (index) {
    ia = index.getX(faceIndex * 3);
    ib = index.getX(faceIndex * 3 + 1);
    ic = index.getX(faceIndex * 3 + 2);
  } else {
    ia = faceIndex * 3;
    ib = faceIndex * 3 + 1;
    ic = faceIndex * 3 + 2;
  }

  const vA = new THREE.Vector3().fromBufferAttribute(posAttr, ia).applyMatrix4(mesh.matrixWorld);
  const vB = new THREE.Vector3().fromBufferAttribute(posAttr, ib).applyMatrix4(mesh.matrixWorld);
  const vC = new THREE.Vector3().fromBufferAttribute(posAttr, ic).applyMatrix4(mesh.matrixWorld);

  return [
    { position: vA, type: 'vertex' },
    { position: vB, type: 'vertex' },
    { position: vC, type: 'vertex' },
    { position: new THREE.Vector3().addVectors(vA, vB).multiplyScalar(0.5), type: 'edge' },
    { position: new THREE.Vector3().addVectors(vB, vC).multiplyScalar(0.5), type: 'edge' },
    { position: new THREE.Vector3().addVectors(vA, vC).multiplyScalar(0.5), type: 'edge' },
    { position: new THREE.Vector3().addVectors(vA, vB).add(vC).multiplyScalar(1 / 3), type: 'face' },
  ];
}

/**
 * Deduplicate candidates that are within `eps` world-space distance of each other.
 * Prefers vertex > edge > face when merging.
 */
const TYPE_PRIORITY: Record<string, number> = { vertex: 0, edge: 1, face: 2 };

function dedup(candidates: SnapCandidate[], eps = 0.01): SnapCandidate[] {
  const out: SnapCandidate[] = [];
  for (const c of candidates) {
    let merged = false;
    for (const o of out) {
      if (c.position.distanceTo(o.position) < eps) {
        // keep higher priority type
        if (TYPE_PRIORITY[c.type] < TYPE_PRIORITY[o.type]) o.type = c.type;
        merged = true;
        break;
      }
    }
    if (!merged) out.push(c);
  }
  return out;
}

const MAX_SURFACE_POINTS = 100;
const FEATURE_ANGLE_DEG = 15; // edges sharper than this are "features"

/**
 * Compute snap candidates for a mesh, limited to geometrically interesting
 * points (corners, sharp edges) rather than every vertex on smooth surfaces.
 * Falls back to spatial thinning if feature points still exceed MAX_SURFACE_POINTS.
 */
export function allMeshCandidates(mesh: THREE.Mesh): SnapPoint[] {
  const geometry = mesh.geometry as THREE.BufferGeometry;
  const posAttr = geometry.getAttribute('position') as THREE.BufferAttribute;
  const index = geometry.getIndex();
  if (!posAttr) return [];

  const vertexCount = posAttr.count;

  // --- Collect world-space vertex positions ---
  const vertPositions: THREE.Vector3[] = [];
  for (let i = 0; i < vertexCount; i++) {
    vertPositions.push(
      new THREE.Vector3().fromBufferAttribute(posAttr, i).applyMatrix4(mesh.matrixWorld),
    );
  }

  // --- Build triangle list and compute face normals ---
  const triangles: [number, number, number][] = [];
  const faceNormals: THREE.Vector3[] = [];
  const tmpAB = new THREE.Vector3();
  const tmpAC = new THREE.Vector3();

  const triCount = index ? index.count / 3 : vertexCount / 3;
  for (let t = 0; t < triCount; t++) {
    let ia: number, ib: number, ic: number;
    if (index) {
      ia = index.getX(t * 3);
      ib = index.getX(t * 3 + 1);
      ic = index.getX(t * 3 + 2);
    } else {
      ia = t * 3;
      ib = t * 3 + 1;
      ic = t * 3 + 2;
    }
    triangles.push([ia, ib, ic]);
    tmpAB.subVectors(vertPositions[ib], vertPositions[ia]);
    tmpAC.subVectors(vertPositions[ic], vertPositions[ia]);
    faceNormals.push(new THREE.Vector3().crossVectors(tmpAB, tmpAC).normalize());
  }

  // --- Build edge → adjacent face indices map ---
  const edgeFaceMap = new Map<string, number[]>();
  const edgeKey = (a: number, b: number) => (a < b ? `${a}-${b}` : `${b}-${a}`);

  for (let t = 0; t < triangles.length; t++) {
    const [ia, ib, ic] = triangles[t];
    for (const [ea, eb] of [[ia, ib], [ib, ic], [ia, ic]] as [number, number][]) {
      const key = edgeKey(ea, eb);
      let faces = edgeFaceMap.get(key);
      if (!faces) {
        faces = [];
        edgeFaceMap.set(key, faces);
      }
      faces.push(t);
    }
  }

  // --- Identify feature edges (sharp angle or boundary) and their vertices ---
  const featureAngleRad = FEATURE_ANGLE_DEG * (Math.PI / 180);
  const featureEdgeKeys = new Set<string>();
  const featureVertexIndices = new Set<number>();

  for (const [key, faces] of edgeFaceMap) {
    let isFeature = false;
    if (faces.length === 1) {
      // Boundary edge – always a feature
      isFeature = true;
    } else if (faces.length >= 2) {
      const angle = faceNormals[faces[0]].angleTo(faceNormals[faces[1]]);
      if (angle > featureAngleRad) {
        isFeature = true;
      }
    }
    if (isFeature) {
      featureEdgeKeys.add(key);
      const parts = key.split('-');
      featureVertexIndices.add(Number(parts[0]));
      featureVertexIndices.add(Number(parts[1]));
    }
  }

  // --- Build candidates from feature vertices + feature edge midpoints ---
  const candidates: SnapCandidate[] = [];
  for (const i of featureVertexIndices) {
    candidates.push({ position: vertPositions[i], type: 'vertex' });
  }
  for (const key of featureEdgeKeys) {
    const parts = key.split('-');
    const a = Number(parts[0]);
    const b = Number(parts[1]);
    const mid = new THREE.Vector3().addVectors(vertPositions[a], vertPositions[b]).multiplyScalar(0.5);
    candidates.push({ position: mid, type: 'edge' });
  }

  // --- Deduplicate coincident positions (UV seams, shared vertices) ---
  let unique = dedup(candidates);

  // --- Spatial thinning if still too many points ---
  if (unique.length > MAX_SURFACE_POINTS) {
    unique = thinCandidates(unique, MAX_SURFACE_POINTS);
  }

  return unique.map((c) => ({
    position: [c.position.x, c.position.y, c.position.z] as [number, number, number],
    type: c.type,
  }));
}

/**
 * Greedy spatial filter: keep at most `maxCount` candidates, preferring
 * vertices over edges, dropping points that are too close together.
 */
function thinCandidates(candidates: SnapCandidate[], maxCount: number): SnapCandidate[] {
  // Sort by type priority so vertices survive the cull
  candidates.sort((a, b) => TYPE_PRIORITY[a.type] - TYPE_PRIORITY[b.type]);

  // Compute bounding box diagonal
  const box = new THREE.Box3();
  for (const c of candidates) box.expandByPoint(c.position);
  const diagonal = box.min.distanceTo(box.max);
  if (diagonal === 0) return candidates.slice(0, maxCount);

  let minSpacing = diagonal / Math.sqrt(maxCount);
  let result: SnapCandidate[] = candidates;

  for (let attempt = 0; attempt < 10; attempt++) {
    result = [];
    for (const c of candidates) {
      let tooClose = false;
      for (const kept of result) {
        if (c.position.distanceTo(kept.position) < minSpacing) {
          tooClose = true;
          break;
        }
      }
      if (!tooClose) result.push(c);
    }
    if (result.length <= maxCount) return result;
    minSpacing *= 1.2;
  }

  return result;
}

export interface SnapResult {
  best: SnapPoint;
  candidates: SnapPoint[];
}

/**
 * Given raycast hits, find the best snap and collect nearby candidates
 * from multiple faces within a fixed screen-space radius.
 */
export function findBestSnap(
  hits: THREE.Intersection[],
  mouseX: number,
  mouseY: number,
  camera: THREE.Camera,
  canvasWidth: number,
  canvasHeight: number,
  snapThresholdPx = 15,
  candidateRadiusPx = 80,
  maxFaces = 8,
): SnapResult | null {
  if (hits.length === 0) return null;

  // Gather raw candidates from up to maxFaces hit faces
  const allRaw: SnapCandidate[] = [];
  const faceCount = Math.min(hits.length, maxFaces);
  for (let i = 0; i < faceCount; i++) {
    allRaw.push(...faceCandidates(hits[i]));
  }

  // Deduplicate (shared verts/edges between adjacent triangles)
  const unique = dedup(allRaw);

  // Filter to candidates within candidateRadiusPx of cursor on screen
  const filtered: { c: SnapCandidate; screenDist: number }[] = [];
  for (const c of unique) {
    const screen = toScreen(c.position, camera, canvasWidth, canvasHeight);
    const dx = screen.x - mouseX;
    const dy = screen.y - mouseY;
    const dist = Math.sqrt(dx * dx + dy * dy);
    if (dist < candidateRadiusPx) {
      filtered.push({ c, screenDist: dist });
    }
  }

  // Convert filtered to SnapPoint[]
  const candidatePoints: SnapPoint[] = filtered.map(({ c }) => ({
    position: [c.position.x, c.position.y, c.position.z] as [number, number, number],
    type: c.type,
  }));

  // Find best snap (closest within snapThresholdPx)
  let bestDist = snapThresholdPx;
  let bestCandidate: SnapCandidate | null = null;
  for (const { c, screenDist } of filtered) {
    if (screenDist < bestDist) {
      bestDist = screenDist;
      bestCandidate = c;
    }
  }

  if (bestCandidate) {
    const p = bestCandidate.position;
    return {
      best: { position: [p.x, p.y, p.z], type: bestCandidate.type },
      candidates: candidatePoints,
    };
  }

  // Fallback: exact hit point on the first intersection
  const hp = hits[0].point;
  return {
    best: { position: [hp.x, hp.y, hp.z], type: 'face' },
    candidates: candidatePoints,
  };
}

/**
 * Format a distance in mm to a human-readable string.
 */
export function formatDistance(mm: number): string {
  if (mm >= 1000) return `${(mm / 1000).toFixed(3)} m`;
  if (mm >= 10) return `${mm.toFixed(1)} mm`;
  if (mm >= 1) return `${mm.toFixed(2)} mm`;
  return `${mm.toFixed(3)} mm`;
}
