import { Component, type ErrorInfo, type ReactNode } from 'react';

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
          <button onClick={() => this.setState({ error: null })} style={{ marginTop: 12 }}>
            Try again
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
