/**
 * Curated Lucide icon set for agents, plus name → component resolution.
 *
 * agent.icon persists the Lucide kebab-case name (e.g. "bot"). The picker grid
 * (`AGENT_ICONS`) and renderers (`resolveAgentIcon`) share this module so the
 * mapping cannot drift; empty/unknown names fall back to Bot.
 */

import {
  Bot,
  Sparkles,
  Brain,
  Code,
  Terminal,
  MessageCircle,
  MessageSquare,
  MessagesSquare,
  Cpu,
  Rocket,
  Zap,
  BookOpen,
  PenTool,
  Search,
  Database,
  Globe,
  Bug,
  GraduationCap,
  Briefcase,
  Palette,
  Camera,
  Music,
  Calculator,
  Compass,
  Feather,
  Heart,
  Star,
  Shield,
  Lightbulb,
  WandSparkles,
  BotMessageSquare,
  Blocks,
} from "@lucide/svelte";

/** All Lucide icons share one shape; Bot's type stands in for it (avoids Component variance issues). */
export type LucideIcon = typeof Bot;

export interface AgentIconOption {
  /** Kebab-case Lucide name persisted to agent.icon. */
  name: string;
  Icon: LucideIcon;
}

/** Picker options; array order is the grid order. */
export const AGENT_ICONS: AgentIconOption[] = [
  { name: "bot", Icon: Bot },
  { name: "bot-message-square", Icon: BotMessageSquare },
  { name: "sparkles", Icon: Sparkles },
  { name: "wand-sparkles", Icon: WandSparkles },
  { name: "brain", Icon: Brain },
  { name: "lightbulb", Icon: Lightbulb },
  { name: "code", Icon: Code },
  { name: "terminal", Icon: Terminal },
  { name: "cpu", Icon: Cpu },
  { name: "blocks", Icon: Blocks },
  { name: "message-circle", Icon: MessageCircle },
  { name: "message-square", Icon: MessageSquare },
  { name: "messages-square", Icon: MessagesSquare },
  { name: "book-open", Icon: BookOpen },
  { name: "pen-tool", Icon: PenTool },
  { name: "feather", Icon: Feather },
  { name: "search", Icon: Search },
  { name: "database", Icon: Database },
  { name: "globe", Icon: Globe },
  { name: "bug", Icon: Bug },
  { name: "graduation-cap", Icon: GraduationCap },
  { name: "briefcase", Icon: Briefcase },
  { name: "palette", Icon: Palette },
  { name: "camera", Icon: Camera },
  { name: "music", Icon: Music },
  { name: "calculator", Icon: Calculator },
  { name: "compass", Icon: Compass },
  { name: "rocket", Icon: Rocket },
  { name: "zap", Icon: Zap },
  { name: "shield", Icon: Shield },
  { name: "heart", Icon: Heart },
  { name: "star", Icon: Star },
];

export const DEFAULT_AGENT_ICON: LucideIcon = Bot;

const ICON_BY_NAME = new Map<string, LucideIcon>(
  AGENT_ICONS.map((o) => [o.name, o.Icon]),
);

export function resolveAgentIcon(name?: string | null): LucideIcon {
  return (name ? ICON_BY_NAME.get(name) : undefined) ?? DEFAULT_AGENT_ICON;
}
