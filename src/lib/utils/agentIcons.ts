/**
 * Agent 图标：精选 Lucide 图标集 + 名字→组件解析
 *
 * agent.icon 持久化为 Lucide 的 kebab-case 图标名（如 "bot" / "sparkles"）。
 * 选择器与渲染共用本模块：`AGENT_ICONS` 供表单网格，`resolveAgentIcon` 供列表 /
 * 侧栏按名反查组件（未命中 / 为空回退到 Bot），二者同源避免映射漂移。
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

/** 所有 Lucide 图标组件同型，用 Bot 的类型统一约束（规避 Component 变型问题）。 */
export type LucideIcon = typeof Bot;

export interface AgentIconOption {
  /** 持久化到 agent.icon 的 kebab-case 图标名。 */
  name: string;
  Icon: LucideIcon;
}

/** 表单可选图标（顺序即网格呈现顺序）。 */
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

/** 缺省 / 未识别图标名时使用的图标。 */
export const DEFAULT_AGENT_ICON: LucideIcon = Bot;

const ICON_BY_NAME = new Map<string, LucideIcon>(
  AGENT_ICONS.map((o) => [o.name, o.Icon]),
);

/** 把 agent.icon 名解析为图标组件；空 / 未识别回退到默认 Bot。 */
export function resolveAgentIcon(name?: string | null): LucideIcon {
  return (name ? ICON_BY_NAME.get(name) : undefined) ?? DEFAULT_AGENT_ICON;
}
