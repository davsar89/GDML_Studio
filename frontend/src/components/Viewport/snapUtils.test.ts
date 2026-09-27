import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import { findBestSnap } from './snapUtils';

/** Snap candidates under the centre of a 100 px view of a 10 mm box, 45 mm in front of it. */
function candidateDepths(transparent: boolean) {
  const box = new THREE.Mesh(new THREE.BoxGeometry(10, 10, 10), new THREE.MeshBasicMaterial({ side: THREE.DoubleSide, transparent }));
  const camera = new THREE.PerspectiveCamera(50, 1, 0.1, 1000);
  camera.position.set(0, 0, 50);
  camera.updateMatrixWorld();
  box.updateMatrixWorld();
  const raycaster = new THREE.Raycaster();
  raycaster.setFromCamera(new THREE.Vector2(0.01, 0.02), camera);
  const result = findBestSnap(raycaster.intersectObject(box), 50.5, 49, camera, 100, 100);
  return new Set(result?.candidates.map((c) => Math.round(c.position[2])));
}

describe('findBestSnap', () => {
  it('an opaque box only offers front-face candidates', () => {
    expect(candidateDepths(false)).toEqual(new Set([5]));
  });

  it('a see-through box also offers the faces behind', () => {
    expect(candidateDepths(true)).toEqual(new Set([5, -5]));
  });
});
