// T3 Code 9c161462c9 client-runtime state/threadExecution.ts (MIT, LICENSE-T3).
import { formatDuration } from '../timeline/timeline-worklog';
import { obj } from '../client/domain';
import { activeRun } from '../client/protocol';
import type { T3Client } from '../client/client';
import { runlessWorkStartedAt } from '../timeline/timeline-inspect';
export type ProviderGoal = { objective: string; status: 'active' | 'paused' | 'blocked' | 'usage_limited' | 'budget_limited' | 'complete';
  tokenBudget?: number | null; tokensUsed?: number; timeUsedSeconds?: number; checks?: number };

export interface ProviderGoalPresentation {
  readonly title: string;
  readonly objective: string;
  readonly usage: string | null;
  readonly canResume: boolean;
}

const PROVIDER_GOAL_TITLES: Record<ProviderGoal["status"], string> = {
  active: "Pursuing goal",
  paused: "Goal paused",
  blocked: "Goal blocked",
  usage_limited: "Goal hit a usage limit",
  budget_limited: "Goal reached its token budget",
  complete: "Goal complete",
};

function formatGoalTokens(tokens: number): string {
  if (tokens < 1_000) return `${tokens}`;
  const thousands = Math.round(tokens / 1_000);
  if (thousands < 1_000) return `${thousands}k`;
  return `${(tokens / 1_000_000).toFixed(1).replace(/\.0$/, "")}m`;
}

export function presentProviderGoal(
  goal: ProviderGoal,
  working: boolean,
): ProviderGoalPresentation {
  const usage: Array<string> = [];
  if (goal.tokensUsed !== undefined && goal.tokensUsed > 0) {
    usage.push(
      goal.tokenBudget == null
        ? `${formatGoalTokens(goal.tokensUsed)} tokens`
        : `${formatGoalTokens(goal.tokensUsed)} / ${formatGoalTokens(goal.tokenBudget)} tokens`,
    );
  }
  if (goal.timeUsedSeconds !== undefined && goal.timeUsedSeconds >= 60) {
    usage.push(formatDuration(goal.timeUsedSeconds * 1_000));
  }
  if (goal.checks !== undefined && goal.checks > 0) {
    usage.push(`${goal.checks} ${goal.checks === 1 ? "check" : "checks"}`);
  }
  return {
    title: goal.status === "active" && !working ? "Goal set" : PROVIDER_GOAL_TITLES[goal.status],
    objective: goal.objective,
    usage: usage.length === 0 ? null : usage.join(" · "),
    canResume: goal.status !== "active" && goal.status !== "complete",
  };
}

export function providerGoal(value: unknown): ProviderGoal | null {
  const goal = obj(value), status = goal.status;
  if (typeof goal.objective !== 'string' || !goal.objective.trim()
    || !(status === 'active' || status === 'paused' || status === 'blocked' || status === 'usage_limited' || status === 'budget_limited' || status === 'complete')) return null;
  const finite = (value: unknown) => typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : undefined;
  return { objective: goal.objective, status, tokensUsed: finite(goal.tokensUsed), tokenBudget: goal.tokenBudget === null ? null : finite(goal.tokenBudget), timeUsedSeconds: finite(goal.timeUsedSeconds), checks: finite(goal.checks) };
}

/** ChatView isWorking includes a provider-native child whose root turn has no app run. */
export function providerGoalWorking(client: T3Client): boolean {
  return !!activeRun(client.projection) || runlessWorkStartedAt(client.projection) !== ''
    || client.busy && client.pending?.threadId === client.threadId && obj(client.pending.payload).type === 'message.dispatch';
}
