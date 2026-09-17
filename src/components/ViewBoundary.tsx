import { ArrowsClockwise, Warning } from "@phosphor-icons/react";
import { Component, type ErrorInfo, type ReactNode } from "react";

import { Button } from "./Button";

interface State {
  error: Error | null;
}

/**
 * Contains a rendering failure to the view it happened in, with a way to recover, instead of
 * unmounting the whole window.
 */
export class ViewBoundary extends Component<{ children: ReactNode; name: string }, State> {
  override state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  override componentDidCatch(_error: Error, _info: ErrorInfo): void {
    // The fallback UI is the report; nothing is sent anywhere.
  }

  override render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    return (
      <div role="alert" className="flex h-full items-center justify-center p-8">
        <div className="flex max-w-[48ch] items-start gap-3">
          <Warning size={16} weight="bold" className="mt-0.5 shrink-0 text-danger" />
          <div className="flex flex-col gap-1">
            <p className="font-medium text-fg">{this.props.name} stopped responding</p>
            <p className="selectable text-fg-muted">
              The view hit an unexpected problem while drawing and was paused. Live monitoring in other views is not
              affected. Details: {error.message}
            </p>
            <div className="mt-2">
              <Button icon={<ArrowsClockwise size={14} />} onClick={() => this.setState({ error: null })}>
                Reload view
              </Button>
            </div>
          </div>
        </div>
      </div>
    );
  }
}
