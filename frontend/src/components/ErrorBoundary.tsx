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

  // Clearing `error` alone would re-render the same children against the store
  // state that just threw, so getDerivedStateFromError would catch again at
  // once. Drop the loaded document first: `reset()` disposes the geometries and
  // empties the document, which is a state the tree is known to render.
  handleRetry = () => {
    useAppStore.getState().reset();
    this.setState({ error: null });
  };

  render() {
    const { error } = this.state;
    if (error) {
      return (
        <div className="fatal-error" role="alert">
          <h2>Something went wrong</h2>
          <p className="validation-error">{error.message}</p>
          <p className="muted">
            &ldquo;Try again&rdquo; closes the current document and returns to the
            empty view. Any unsaved edits are lost.
          </p>
          <div className="actions">
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
