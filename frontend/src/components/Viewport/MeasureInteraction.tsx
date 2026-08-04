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
