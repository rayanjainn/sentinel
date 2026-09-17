// Agent state for the chat panel and settings, fed by `sentinel:agent` events.
import type { UnlistenFn } from "@tauri-apps/api/event";
import { create } from "zustand";

import type { Action } from "../../bindings/Action";
import type { AgentSettings } from "../../bindings/AgentSettings";
import type { ErrorPayload } from "../../bindings/ErrorPayload";
import type { KeyStatus } from "../../bindings/KeyStatus";
import type { ModelInfo } from "../../bindings/ModelInfo";
import type { OllamaStatus } from "../../bindings/OllamaStatus";
import type { Plan } from "../../bindings/Plan";
import type { ProviderDescriptor } from "../../bindings/ProviderDescriptor";
import type { ProviderId } from "../../bindings/ProviderId";
import type { ProviderStatus } from "../../bindings/ProviderStatus";
import { toPayload } from "../../lib/errors";
import { api, on } from "../../lib/ipc";
import { decisionsFor, emptyReview, type PlanReview, type ReviewDecision } from "./model";
import { addUserMessage, applyEvent, emptyChat, fromTranscript, type ChatState } from "./transcript";

interface AgentStore {
  descriptors: ProviderDescriptor[] | null;
  settings: AgentSettings | null;
  statuses: ProviderStatus[] | null;
  ollama: OllamaStatus | null;
  models: Partial<Record<ProviderId, ModelInfo[]>>;
  modelErrors: Partial<Record<ProviderId, ErrorPayload>>;
  loadError: ErrorPayload | null;

  conversationId: string | null;
  chat: ChatState;
  sendError: ErrorPayload | null;

  reviews: Record<string, PlanReview>;
  runningPlans: Record<string, boolean>;
  planErrors: Record<string, ErrorPayload | undefined>;

  bootstrap: () => Promise<void>;
  refreshStatus: () => Promise<void>;
  loadModels: (provider: ProviderId) => Promise<void>;
  saveSettings: (settings: AgentSettings) => Promise<ErrorPayload | null>;
  selectProvider: (provider: ProviderId) => Promise<void>;
  selectModel: (provider: ProviderId, model: string) => Promise<void>;
  setApiKey: (provider: ProviderId, key: string) => Promise<KeyStatus | ErrorPayload>;
  deleteApiKey: (provider: ProviderId) => Promise<ErrorPayload | null>;

  newConversation: () => Promise<void>;
  send: (text: string) => Promise<boolean>;
  cancel: () => Promise<void>;
  dismissSendError: () => void;

  decide: (planId: string, actionId: string, decision: ReviewDecision | null) => void;
  confirm: (planId: string, actionId: string, text: string) => void;
  runPlan: (plan: Plan) => Promise<void>;
  revise: (planId: string, actionId: string, action: Action) => Promise<ErrorPayload | null>;
}

let listening: Promise<UnlistenFn> | null = null;

export const useAgent = create<AgentStore>()((set, get) => {
  const applyPlan = (plan: Plan) => set((s) => ({ chat: applyEvent(s.chat, { type: "planUpdated", plan }) }));

  const listen = () => {
    listening ??= on("sentinel:agent", (payload) => {
      if (payload.conversationId !== get().conversationId) return;
      set((s) => ({ chat: applyEvent(s.chat, payload.event) }));
    });
  };

  const startConversation = async () => {
    const id = await api.agent.newConversation();
    set({ conversationId: id, chat: emptyChat, reviews: {}, runningPlans: {}, planErrors: {}, sendError: null });
    return id;
  };

  return {
    descriptors: null,
    settings: null,
    statuses: null,
    ollama: null,
    models: {},
    modelErrors: {},
    loadError: null,
    conversationId: null,
    chat: emptyChat,
    sendError: null,
    reviews: {},
    runningPlans: {},
    planErrors: {},

    bootstrap: async () => {
      listen();
      try {
        const [descriptors, settings] = await Promise.all([api.agent.listProviders(), api.agent.getSettings()]);
        set({ descriptors, settings, loadError: null });
      } catch (error) {
        set({ loadError: toPayload(error) });
        return;
      }
      void get().refreshStatus();
      const { conversationId } = get();
      if (!conversationId) {
        await startConversation().catch((error: unknown) => set({ loadError: toPayload(error) }));
        return;
      }
      try {
        const transcript = await api.agent.getTranscript(conversationId);
        set((s) => (s.chat.running ? {} : { chat: fromTranscript(transcript) }));
      } catch {
        await startConversation().catch((error: unknown) => set({ loadError: toPayload(error) }));
      }
    },

    refreshStatus: async () => {
      const [statuses, ollama] = await Promise.all([
        api.agent.providerStatus().catch(() => null),
        api.agent.ollamaStatus().catch(() => null),
      ]);
      set({ statuses, ollama });
      const provider = get().settings?.activeProvider;
      if (provider && provider !== "ollama") void get().loadModels(provider);
    },

    loadModels: async (provider) => {
      try {
        const models = await api.agent.listModels(provider);
        set((s) => ({ models: { ...s.models, [provider]: models }, modelErrors: { ...s.modelErrors, [provider]: undefined } }));
      } catch (error) {
        set((s) => ({ modelErrors: { ...s.modelErrors, [provider]: toPayload(error) } }));
      }
    },

    saveSettings: async (settings) => {
      try {
        const saved = await api.agent.updateSettings(settings);
        const descriptors = await api.agent.listProviders().catch(() => get().descriptors);
        set({ settings: saved, descriptors });
        return null;
      } catch (error) {
        return toPayload(error);
      }
    },

    selectProvider: async (provider) => {
      const current = get().settings;
      if (!current || current.activeProvider === provider) return;
      await get().saveSettings({ ...current, activeProvider: provider });
      void get().refreshStatus();
      void get().loadModels(provider);
    },

    selectModel: async (provider, model) => {
      const current = get().settings;
      if (!current) return;
      await get().saveSettings({ ...current, selectedModels: { ...current.selectedModels, [provider]: model } });
    },

    setApiKey: async (provider, key) => {
      try {
        const status = await api.agent.setApiKey(provider, key);
        void get().refreshStatus();
        if (status.state === "valid" || status.state === "unverified") void get().loadModels(provider);
        return status;
      } catch (error) {
        return toPayload(error);
      }
    },

    deleteApiKey: async (provider) => {
      try {
        await api.agent.deleteApiKey(provider);
        set((s) => ({ models: { ...s.models, [provider]: undefined } }));
        void get().refreshStatus();
        return null;
      } catch (error) {
        return toPayload(error);
      }
    },

    newConversation: async () => {
      const { conversationId, chat } = get();
      if (conversationId && chat.running) await api.agent.cancel(conversationId).catch(() => undefined);
      await startConversation().catch((error: unknown) => set({ loadError: toPayload(error) }));
    },

    send: async (text) => {
      const message = text.trim();
      if (!message || get().chat.running) return false;
      let conversationId = get().conversationId ?? (await startConversation());
      const before = get().chat;
      set({ chat: { ...addUserMessage(before, message), running: true }, sendError: null });
      try {
        await api.agent.sendMessage(conversationId, message);
        return true;
      } catch (error) {
        let payload = toPayload(error);
        // The backend keeps conversations in memory; after an app restart start a fresh one.
        if (payload.code === "invalidInput" && payload.detail.includes("no longer exists")) {
          try {
            conversationId = await startConversation();
            set({ chat: { ...addUserMessage(emptyChat, message), running: true } });
            await api.agent.sendMessage(conversationId, message);
            return true;
          } catch (retryError) {
            payload = toPayload(retryError);
          }
        }
        set({ chat: before, sendError: payload });
        return false;
      }
    },

    cancel: async () => {
      const { conversationId } = get();
      if (conversationId) await api.agent.cancel(conversationId).catch(() => undefined);
    },

    dismissSendError: () => set({ sendError: null }),

    decide: (planId, actionId, decision) =>
      set((s) => {
        const review = s.reviews[planId] ?? emptyReview;
        const decisions = { ...review.decisions };
        if (decision) decisions[actionId] = decision;
        else delete decisions[actionId];
        return { reviews: { ...s.reviews, [planId]: { ...review, decisions } } };
      }),

    confirm: (planId, actionId, text) =>
      set((s) => {
        const review = s.reviews[planId] ?? emptyReview;
        return {
          reviews: { ...s.reviews, [planId]: { ...review, confirmations: { ...review.confirmations, [actionId]: text } } },
        };
      }),

    runPlan: async (plan) => {
      const review = get().reviews[plan.id] ?? emptyReview;
      set((s) => ({ runningPlans: { ...s.runningPlans, [plan.id]: true }, planErrors: { ...s.planErrors, [plan.id]: undefined } }));
      try {
        applyPlan(await api.agent.executePlan(plan.id, decisionsFor(plan, review)));
      } catch (error) {
        set((s) => ({ planErrors: { ...s.planErrors, [plan.id]: toPayload(error) } }));
      } finally {
        set((s) => ({ runningPlans: { ...s.runningPlans, [plan.id]: false } }));
      }
    },

    revise: async (planId, actionId, action) => {
      try {
        applyPlan(await api.agent.revisePlanAction(planId, actionId, action));
        // A refreshed preview needs a fresh decision.
        set((s) => {
          const review = s.reviews[planId] ?? emptyReview;
          const decisions = { ...review.decisions };
          const confirmations = { ...review.confirmations };
          delete decisions[actionId];
          delete confirmations[actionId];
          return { reviews: { ...s.reviews, [planId]: { decisions, confirmations } } };
        });
        return null;
      } catch (error) {
        return toPayload(error);
      }
    },
  };
});
