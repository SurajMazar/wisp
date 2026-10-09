import { useEffect, type ReactNode } from "react";
import { ipc } from "../../services/ipc";
import { useAssistantStore } from "../../stores/assistantStore";
import { useConnectionStore } from "../../stores/connectionStore";
import { useAgentStore } from "../../stores/agentStore";
import { useTeachStore } from "../../stores/teachStore";
import { useToolStore } from "../../stores/toolStore";
import { useConversationStore } from "../../stores/conversationStore";
import { useFocusStore } from "../../stores/focusStore";
import { useModelsStore } from "../../stores/modelsStore";
import { useInvocationStore } from "../../stores/invocationStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useVoiceStore } from "../../stores/voiceStore";
import { applyAppearance } from "../../lib/theme";

/** Connects stores to the core: initial loads, event subscriptions, theming. */
export function Bootstrap({ children }: { children: ReactNode }) {
  const appearance = useSettingsStore((s) => s.settings?.appearance);

  useEffect(() => {
    const client = ipc();
    const offs = [
      client.on("settings.changed", (s) => useSettingsStore.getState().receive(s)),
      client.on("invocation.statusChanged", (st) => useInvocationStore.getState().receiveStatus(st)),
      client.on("invocation.started", (e) => useAssistantStore.getState().invoked(e.source, e.behavior)),
      client.on("assistant.stateChanged", (e) => useAssistantStore.getState().receive(e.state, e.expanded)),
      client.on("focus.completed", (r) => useFocusStore.getState().receive(r)),
      client.on("focus.ended", (e) => useFocusStore.getState().ended(e)),
      client.on("focus.cleared", () => useFocusStore.getState().cleared()),
      client.on("models.pullProgress", (p) => useModelsStore.getState().receivePull(p)),
      client.on("conversation.delta", (d) => useConversationStore.getState().receiveDelta(d)),
      client.on("conversation.reasoningDelta", (d) => useConversationStore.getState().receiveReasoning(d)),
      client.on("conversation.messageUpdated", (m) => useConversationStore.getState().receiveMessage(m)),
      client.on("conversation.updated", (c) => useConversationStore.getState().receiveSummary(c)),
      client.on("conversation.messageRemoved", (m) => useConversationStore.getState().receiveRemoved(m)),
      client.on("tool.approvalRequested", (r) => useToolStore.getState().receiveRequest(r)),
      client.on("tool.approvalResolved", (r) => useToolStore.getState().receiveResolved(r.id)),
      client.on("tool.callUpdated", (r) => useToolStore.getState().receiveCall(r)),
      client.on("teach.updated", (l) => useTeachStore.getState().receive(l)),
      client.on("agent.runUpdated", (r) => useAgentStore.getState().receiveRun(r)),
      client.on("voice.level", (l) => useVoiceStore.getState().setLevel(l.level)),
      client.on("voice.modelProgress", (p) => useVoiceStore.getState().receiveProgress(p)),
      client.on("voice.asked", (a) => {
        const conv = useConversationStore.getState();
        conv.setPanel(a.conversationId);
        void conv.open(a.conversationId);
      }),
      client.on("core.ready", (info) => {
        useConnectionStore.getState().setConnected(info);
        void useSettingsStore.getState().load();
      }),
    ];
    client
      .call("app.info", {}, { timeoutMs: 5_000 })
      .then((info) => useConnectionStore.getState().setConnected(info))
      .catch(() => useConnectionStore.getState().setUnavailable());
    void useSettingsStore.getState().load();
    void useInvocationStore.getState().loadStatus();
    void useAssistantStore.getState().load();
    void useFocusStore.getState().load();
    return () => offs.forEach((off) => off());
  }, []);

  useEffect(() => (appearance ? applyAppearance(appearance) : undefined), [appearance]);

  return children;
}
