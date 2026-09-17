// Public surface of the shared action flow. `ActionPreviewBody` and `OutcomeSummary` are pure,
// prop-driven components the agent's plan cards reuse.
export { ActionDialog } from "./ActionDialog";
export { ActionPreviewBody } from "./ActionPreviewBody";
export { OutcomeSummary } from "./OutcomeSummary";
export { cancelAction, confirmAction, runAction, showOutcome, useActionFlow, type FlowState } from "./flow";
export { confirmationPhrase, confirmLabel, doneLabel, riskLabel } from "./labels";
