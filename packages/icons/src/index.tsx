/**
 * Central icon registry. App code imports icons from here, never from lucide directly,
 * so the icon set stays consistent and swappable.
 */
export {
  Home as HomeIcon,
  MessagesSquare as ConversationsIcon,
  Bot as AgentsIcon,
  BookOpen as SkillsIcon,
  CalendarClock as RoutinesIcon,
  Brain as MemoryIcon,
  FolderOpen as FilesIcon,
  Cpu as ModelsIcon,
  AudioLines as VoiceIcon,
  ShieldCheck as PermissionsIcon,
  Settings2 as SettingsIcon,
  Lock as PrivacyIcon,
  Palette as AppearanceIcon,
  SlidersHorizontal as GeneralIcon,
  Plug as DisconnectedIcon,
  Check as CheckIcon,
  ArrowUp as SendIcon,
  ArrowDown as ArrowDownIcon,
  Square as StopIcon,
  Plus as PlusIcon,
  Copy as CopyIcon,
  RotateCcw as RegenerateIcon,
  Volume2 as SpeakIcon,
  ChevronDown as ChevronDownIcon,
  Mic as MicIcon,
  X as CloseIcon,
  Scan as AreaIcon,
  MousePointerClick as PointIcon,
  AppWindow as WindowIcon,
  Monitor as ScreenIcon,
  Sparkles as SparklesIcon,
  Cpu as ChipIcon,
  ChevronRight as ChevronRightIcon,
  CircleCheck as StepDoneIcon,
  Lightbulb as ThinkingIcon,
  Activity as ActivityIcon,
  ShieldAlert as RiskIcon,
  Hand as ControlIcon,
  GraduationCap as TeachIcon,
  Zap as ActIcon,
  Pause as PauseIcon,
  Play as PlayIcon,
  Bot as BotIcon,
  Trash2 as DeleteIcon,
  Pencil as EditIcon,
  Circle as StepPendingIcon,
} from "lucide-react";

/** Wisp's mark: a small luminous core with a soft trailing halo. */
export function WispMark({ size = 16, className }: { size?: number; className?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" className={className} aria-hidden="true">
      <defs>
        <radialGradient id="wisp-core" cx="45%" cy="40%" r="60%">
          <stop offset="0%" stopColor="#ffffff" />
          <stop offset="45%" stopColor="#a99cf8" />
          <stop offset="100%" stopColor="#5ec8f2" stopOpacity="0" />
        </radialGradient>
      </defs>
      <circle cx="12" cy="12" r="11" fill="url(#wisp-core)" opacity="0.55" />
      <circle cx="11" cy="11" r="4.2" fill="url(#wisp-core)" />
      <path
        d="M14.5 14.5c2.2 1.4 4.1 1.9 6 1.6"
        stroke="#8b7cf6"
        strokeWidth="1.4"
        strokeLinecap="round"
        fill="none"
        opacity="0.7"
      />
    </svg>
  );
}
