import { Canvas } from '@react-three/fiber';
import { OrbitControls, GizmoHelper, GizmoViewport } from '@react-three/drei';
import Scene from './Scene';
import { useAppStore } from '../../store';

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
        <gridHelper args={[2000, 40, '#333344', '#222233']} />
        <axesHelper args={[200]} />
        <OrbitControls makeDefault />
        <GizmoHelper alignment="bottom-right" margin={[60, 60]}>
          <GizmoViewport />
        </GizmoHelper>
      </Canvas>
    </div>
  );
}
