export interface OpenOptions {
  configPath?: string;
  configToml?: string;
}
export interface SearchOptions {
  k?: number;
  videos?: string[];
  kinds?: Array<"transcript" | "ocr" | "description" | "frame">;
  textOnly?: boolean;
  /** At most this many hits per video. */
  perVideoK?: number;
}
export interface AskOptions {
  videos?: string[];
  sessionId?: string;
  policy?: "agent" | "retrieval-only";
  /** Chat model: a `[providers.*]` name or its model id (default: the `agent_llm` role). */
  model?: string;
  maxTokens?: number;
  maxCostUsd?: number;
  maxWallclockSecs?: number;
  maxToolCalls?: number;
}
export interface Citation {
  type: "citation";
  video_id: string;
  t0: number;
  t1: number;
  kind: string;
}
export interface AskUsage {
  tokens_in: number;
  tokens_out: number;
  cost_usd: number;
  tool_calls: number;
  provider_calls: number;
  wallclock_ms: number;
}
export type AskEvent =
  | { type: "status"; text: string }
  /** `turn`: loop turn (1-based) that issued the call; calls sharing a turn ran concurrently. */
  | { type: "tool_call"; tool: string; args: unknown; turn: number }
  /** `ms`: the tool's own wall time. */
  | { type: "tool_result"; tool: string; summary: string; turn: number; ms: number }
  | { type: "token"; text: string }
  | Citation
  | { type: "done"; partial: boolean; reason: string | null; usage: AskUsage };
export interface Collected {
  text: string;
  citations: Citation[];
  toolCalls: Array<{ type: "tool_call"; tool: string; args: unknown }>;
  usage: AskUsage | null;
  partial: boolean;
  reason: string | null;
}
export interface AskIterable extends AsyncIterable<AskEvent> {
  collect(): Promise<Collected>;
}
export class Index {
  static open(path: string, opts?: OpenOptions): Index;
  static create(path: string, opts?: OpenOptions): Index;
  readonly path: string;
  videos(): Promise<unknown[]>;
  status(): Promise<unknown>;
  timeline(videoId: string, level?: "chapter" | "scene" | "shot"): Promise<unknown[]>;
  search(query: string, opts?: SearchOptions): Promise<{ hits: unknown[]; lists: string[]; grouping: string }>;
  add(source: string, policy?: string, force?: boolean): Promise<unknown[]>;
  ask(question: string, opts?: AskOptions): AskIterable;
}
export function version(): string;
