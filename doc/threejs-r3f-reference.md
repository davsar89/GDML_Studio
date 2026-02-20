# Three.js & React Three Fiber Reference

> For GDML Studio geometry viewer development.
> Sources: [Three.js docs](https://threejs.org/docs/), [R3F docs](https://r3f.docs.pmnd.rs/), [Drei](https://github.com/pmndrs/drei)

---

## 1. BufferGeometry — Custom Geometry from Raw Data

```javascript
const geometry = new THREE.BufferGeometry();

// Positions: 3 floats per vertex (x, y, z)
geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));

// Normals: 3 floats per vertex (nx, ny, nz)
geometry.setAttribute('normal', new THREE.Float32BufferAttribute(normals, 3));

// Index buffer: Uint16 for <65536 vertices, Uint32 for larger
geometry.setIndex(new THREE.Uint32BufferAttribute(indices, 1));

// Required for frustum culling and raycasting
geometry.computeBoundingBox();
geometry.computeBoundingSphere();
```

**Updating attributes:**
```javascript
const posAttr = geometry.getAttribute('position');
posAttr.array[0] = newX;
posAttr.needsUpdate = true;  // flag for GPU re-upload
```

**Disposal:** `geometry.dispose()` — releases GPU buffers. Must be called manually.

---

## 2. InstancedMesh — Efficient Repeated Geometry

Renders thousands of the same geometry in a single draw call.

```javascript
const mesh = new THREE.InstancedMesh(geometry, material, count);

const dummy = new THREE.Object3D();
for (let i = 0; i < count; i++) {
  dummy.position.set(x, y, z);
  dummy.rotation.set(rx, ry, rz);
  dummy.updateMatrix();
  mesh.setMatrixAt(i, dummy.matrix);

  // Optional per-instance color
  mesh.setColorAt(i, new THREE.Color(0xff0000));
}
mesh.instanceMatrix.needsUpdate = true;
```

| Property | Description |
|----------|-------------|
| `instanceMatrix` | Float32Array of 4x4 matrices (16 floats per instance) |
| `instanceColor` | RGB colors (3 floats per instance), created on first `setColorAt` |
| `count` | Number of visible instances (can be set lower than allocated max) |

**Performance:** Reduces N draw calls to 1. Real-world: 9,000 draw calls reduced to 300.

---

## 3. BatchedMesh — Multiple Geometries, Single Draw Call

Available since Three.js r156+. Supports different geometries sharing one material.

```javascript
const batchedMesh = new THREE.BatchedMesh(maxInstances, maxVertices, maxIndices, material);

const boxId = batchedMesh.addGeometry(boxGeometry);
const sphereId = batchedMesh.addGeometry(sphereGeometry);

const inst1 = batchedMesh.addInstance(boxId);
batchedMesh.setMatrixAt(inst1, matrix);
```

**When to use what:**

| Scenario | Technique |
|----------|-----------|
| Same shape, many placements | **InstancedMesh** |
| Mix of shapes, same material | **BatchedMesh** |
| Static background geometry | **Geometry merging** |
| Need individual selection | **Separate meshes** + BVH |

---

## 4. Materials for CAD Viewers

| Material | Lighting Model | Performance | Best For |
|----------|---------------|-------------|----------|
| `MeshStandardMaterial` | PBR | Moderate | Final presentation |
| `MeshPhongMaterial` | Blinn-Phong | Good | Decent quality, less GPU |
| `MeshLambertMaterial` | Diffuse only | Best | Large scenes, preview |

**Key properties:**
```javascript
new THREE.MeshStandardMaterial({
  color: 0x44aa88,
  emissive: 0x000000,
  metalness: 0.0,      // 0=dielectric, 1=metal
  roughness: 0.5,      // 0=mirror, 1=fully rough
  side: THREE.DoubleSide,
  transparent: false,
  opacity: 1.0,
  wireframe: false,
  flatShading: false,
});
```

Share material instances across meshes with the same appearance (enables draw call batching).

---

## 5. Euler Rotations

**Default order:** `'XYZ'` intrinsic — first X, then local Y, then local Z.

**Available:** `'XYZ'`, `'YZX'`, `'ZXY'`, `'XZY'`, `'YXZ'`, `'ZYX'`

```javascript
const euler = new THREE.Euler(rx, ry, rz, 'XYZ');
object.rotation.set(rx, ry, rz);
object.rotation.order = 'ZYX';

// Quaternion (avoids gimbal lock)
const quat = new THREE.Quaternion().setFromEuler(euler);
```

**GDML note:** GDML rotations with Euler order `'XYZ'` in Three.js correspond to the matrix `Rz * Ry * Rx` (extrinsic), which matches GDML's convention.

---

## 6. Scene Graph

```
Object3D (base: position, rotation, scale, matrix, matrixWorld)
  ├─ Scene
  ├─ Group (empty container)
  ├─ Mesh (geometry + material)
  ├─ InstancedMesh
  ├─ Camera
  └─ Light
```

**Key operations:**
```javascript
group.add(child);
group.remove(child);
scene.traverse((obj) => { /* depth-first */ });
scene.traverseVisible((obj) => { /* visible only */ });
scene.getObjectByName('name');
```

**World matrix:** `child.matrixWorld = parent.matrixWorld * child.matrix`

---

## 7. OrbitControls for CAD Viewers

```javascript
const controls = new OrbitControls(camera, renderer.domElement);
controls.enableDamping = true;
controls.dampingFactor = 0.1;
controls.minDistance = 1;
controls.maxDistance = 10000;
controls.target.set(0, 0, 0);
controls.enablePan = true;
controls.screenSpacePanning = true;
// Must call controls.update() in render loop when damping is enabled
```

---

## 8. Performance

### Geometry Merging (static objects)
```javascript
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils';
const merged = mergeGeometries([geom1, geom2, geom3]);
// Caveat: merged objects cannot be individually transformed or picked
```

### Level of Detail (LOD)
```javascript
const lod = new THREE.LOD();
lod.addLevel(highDetail, 0);
lod.addLevel(medDetail, 50);
lod.addLevel(lowDetail, 200);
```

### Frustum Culling
- Built-in and enabled by default (`object.frustumCulled = true`)
- Requires accurate bounding spheres for custom geometries
- Per-object only, not sub-mesh

### Draw Call Targets
- Aim for <100 draw calls for 60fps
- Monitor: `renderer.info.render.calls`, `renderer.info.memory.geometries`

---

## 9. Raycasting / Object Picking

```javascript
const raycaster = new THREE.Raycaster();
raycaster.setFromCamera(pointer, camera);
const intersects = raycaster.intersectObjects(scene.children, true);
// intersects[0]: { object, distance, point, face, faceIndex }
```

**BVH acceleration** (for large scenes):
```javascript
import { MeshBVH, acceleratedRaycast } from 'three-mesh-bvh';
THREE.Mesh.prototype.raycast = acceleratedRaycast;
geometry.boundsTree = new MeshBVH(geometry);
```

---

## 10. Memory Management

```javascript
// Dispose geometry
geometry.dispose();

// Dispose material and its textures
for (const key of Object.keys(material)) {
  if (material[key]?.isTexture) material[key].dispose();
}
material.dispose();

// Full scene cleanup
scene.traverse((obj) => {
  if (obj.geometry) obj.geometry.dispose();
  if (obj.material) {
    if (Array.isArray(obj.material)) obj.material.forEach(m => m.dispose());
    else obj.material.dispose();
  }
});

// Destroy renderer
renderer.dispose();
```

**Rules:**
1. Always `dispose()` before removing objects
2. `scene.clear()` does NOT free GPU memory
3. Share material/geometry instances; dispose only when all refs are gone
4. Call `renderer.dispose()` when destroying the viewer

---

## 11. Lighting for Engineering Viewers

```javascript
// Recommended setup
scene.add(new THREE.AmbientLight(0xffffff, 0.4));

const dir = new THREE.DirectionalLight(0xffffff, 0.8);
dir.position.set(5, 10, 7);
scene.add(dir);

// Optional hemisphere for sky/ground fill
scene.add(new THREE.HemisphereLight(0xffffff, 0x444444, 0.6));
```

---

## 12. React Three Fiber Patterns

### useFrame — Per-Frame Updates
```jsx
useFrame((state, delta) => {
  meshRef.current.rotation.y += delta;
  // NEVER use setState here
});
```

### useThree — Access Renderer State
```jsx
const { gl, scene, camera, size, viewport } = useThree();
// Selective subscription (prevents re-renders):
const camera = useThree((s) => s.camera);
```

### Canvas Component
```jsx
<Canvas
  camera={{ position: [0, 0, 5], fov: 75, near: 0.1, far: 1000 }}
  gl={{ antialias: true }}
  dpr={[1, 2]}
  shadows
  frameloop="always"  // 'always' | 'demand' | 'never'
/>
```

### Performance Patterns
```jsx
// Memoize geometry and materials
const geometry = useMemo(() => new THREE.BoxGeometry(1, 1, 1), []);

// React.memo for expensive components
const Volume = React.memo(function Volume({ geometry, color }) { ... });

// Instancing in R3F
<instancedMesh ref={ref} args={[null, null, count]}>
  <sphereGeometry args={[0.5, 16, 16]} />
  <meshStandardMaterial />
</instancedMesh>
```

### Event Handling
```jsx
<mesh
  onClick={(e) => { e.stopPropagation(); /* pick */ }}
  onPointerOver={(e) => { /* hover */ }}
  onPointerOut={(e) => { /* unhover */ }}
/>
```

### Drei Helpers

| Helper | Purpose |
|--------|---------|
| `<OrbitControls />` | Camera orbit/pan/zoom |
| `<Environment preset="studio" />` | IBL environment |
| `<Stats />` | FPS counter |
| `<Html position={[0,1,0]}>` | HTML in 3D space |
| `<Bvh>` | BVH acceleration for raycasting |
| `<Edges threshold={15} />` | Edge visualization |
| `<Outlines />` | Object outlining |
| `<GizmoHelper />` | XYZ orientation gizmo |
| `<Grid infiniteGrid />` | Ground plane grid |
| `<Detailed distances={[0,50,200]}>` | LOD component |

---

## 13. GDML Viewer-Specific Recommendations

1. **Group identical solids** into InstancedMesh — one per geometry type per material
2. Use **MeshLambertMaterial** for preview, **MeshStandardMaterial** for presentation
3. **LOD** for complex sub-detectors: full mesh close, simplified far
4. **Frustum culling** is automatic; ensure `computeBoundingSphere()` on custom geometries
5. **Share materials** — create a palette, reuse instances
6. **Dispose on scene change** — traverse and dispose all previous geometry/materials
7. Use **Drei `<Bvh>`** for accelerated picking
8. Monitor `renderer.info` to catch geometry/texture leaks
