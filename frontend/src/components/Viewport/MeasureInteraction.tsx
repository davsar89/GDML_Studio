import { useEffect } from 'react';
import { useThree } from '@react-three/fiber';
import { useAppStore } from '../../store';
import { useMeasureRaycast } from './useMeasureRaycast';

export default function MeasureInteraction() {
  useMeasureRaycast();

  const { gl } = useThree();

  useEffect(() => {
    const canvas = gl.domElement;

    const onPointerDown = (e: PointerEvent) => {
      if (e.button !== 0) return; // left click only
      const state = useAppStore.getState();
      if (!state.measureMode || !state.hoverSnap) return;
      state.placeMeasurePoint(state.hoverSnap);
    };

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        const state = useAppStore.getState();
        if (state.measureMode) {
          e.preventDefault();
          state.cancelMeasure();
        }
      }
    };

    canvas.addEventListener('pointerdown', onPointerDown);
    window.addEventListener('keydown', onKeyDown);
    return () => {
      canvas.removeEventListener('pointerdown', onPointerDown);
      window.removeEventListener('keydown', onKeyDown);
    };
  }, [gl]);

  // Cursor style
  useEffect(() => {
    const unsubscribe = useAppStore.subscribe((state) => {
      gl.domElement.style.cursor = state.measureMode ? 'crosshair' : '';
    });
    // Set initial
    gl.domElement.style.cursor = useAppStore.getState().measureMode ? 'crosshair' : '';
    return () => {
      unsubscribe();
      gl.domElement.style.cursor = '';
    };
  }, [gl]);

  return null;
}
