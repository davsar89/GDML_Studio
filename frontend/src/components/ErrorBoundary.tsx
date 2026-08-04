import { Component, type ErrorInfo, type ReactNode } from 'react';
import { useAppStore } from '../store';

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
}

/**
 * Catches render-time errors anywhere in the tree and shows a recoverable
 * message instead of unmounting the whole app to a blank screen.
 */
export default class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('Unhandled UI error:', error, info);
  }

  // Clearing `error` alone re-renders the same children against the same store
  // state that just threw, so getDerivedStateFromError catches again and the
  // message comes straight back -- the button appeared to do nothing. Drop the
  // loaded document first: `reset()` disposes the geometries and returns every
  // slice to its empty state, which is a state the tree is known to render.
  handleRetry = () => {
    useAppStore.getState().reset();
    this.setState({ error: null });
  };

  render() {
    const { error } = this.state;
    if (error) {
      return (
        <div
          style={{
            padding: 24,
            color: '#ff8080',
            fontFamily: 'monospace',
            background: '#1a1a1f',
            height: '100vh',
            boxSizing: 'border-box',
            overflow: 'auto',
          }}
        >
          <h2 style={{ marginTop: 0 }}>Something went wrong</h2>
          <p style={{ whiteSpace: 'pre-wrap' }}>{error.message}</p>
          <p style={{ color: '#8899aa' }}>
            &ldquo;Try again&rdquo; closes the current document and returns to the
            empty view. Any unsaved edits are lost.
          </p>
          <div style={{ display: 'flex', gap: 8, marginTop: 12 }}>
            <button onClick={this.handleRetry}>Try again</button>
            {/* Fallback for an error in code `reset()` cannot clear. */}
            <button onClick={() => window.location.reload()}>Reload page</button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
