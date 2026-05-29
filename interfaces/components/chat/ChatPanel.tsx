"use client";

import { useEffect, useRef, useState } from "react";
import { useTranslations } from "next-intl";
import { useChatStore } from "@/stores/chatStore";

export function ChatPanel() {
  const t = useTranslations();
  const {
    messages,
    isLoading,
    error,
    activeProvider,
    providerLoaded,
    loadActiveProvider,
    sendMessage,
    clearMessages,
    clearError,
  } = useChatStore();

  const [input, setInput] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    loadActiveProvider();
  }, [loadActiveProvider]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = input.trim();
    if (!trimmed || isLoading) return;
    setInput("");
    sendMessage(trimmed);
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit(e);
    }
  }

  // ── Loading active provider ───────────────────────────────────

  if (!providerLoaded) {
    return (
      <div className="flex items-center justify-center h-full text-sm text-zinc-400">
        {t("chat.loading")}
      </div>
    );
  }

  // ── No active provider: setup prompt ──────────────────────────

  if (!activeProvider) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-3 p-6 text-center">
        <span className="text-4xl">💬</span>
        <h2 className="text-lg font-semibold">{t("chat.notConfigured.title")}</h2>
        <p className="text-sm text-zinc-500 max-w-sm">
          {t("chat.notConfigured.descriptionBefore")}{" "}
          <span className="font-mono">{t("chat.notConfigured.settingsPath")}</span>
          {t("chat.notConfigured.descriptionAfter")}
        </p>
      </div>
    );
  }

  // ── Normal chat UI ────────────────────────────────────────────

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-zinc-200 dark:border-zinc-700">
        <div className="text-sm text-zinc-500">
          {t("chat.activeModel", { model: activeProvider.model })}
        </div>
        {messages.length > 0 && (
          <button
            onClick={clearMessages}
            className="text-xs px-2 py-1 text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300"
          >
            {t("chat.clear")}
          </button>
        )}
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {messages.length === 0 && !isLoading && (
          <div className="flex flex-col items-center justify-center h-full gap-2 text-zinc-400">
            <span className="text-3xl">💬</span>
            <p className="text-sm">{t("chat.emptyState")}</p>
          </div>
        )}

        {messages.map((msg, idx) => (
          <div
            key={idx}
            className={`flex ${msg.role === "user" ? "justify-end" : "justify-start"}`}
          >
            <div
              className={`max-w-[80%] rounded-lg px-3 py-2 text-sm whitespace-pre-wrap ${
                msg.role === "user"
                  ? "bg-blue-600 text-white"
                  : "bg-zinc-100 dark:bg-zinc-800 text-zinc-900 dark:text-zinc-100"
              }`}
            >
              {msg.content || (msg.role === "assistant" && isLoading ? "…" : "")}
            </div>
          </div>
        ))}
        <div ref={messagesEndRef} />
      </div>

      {/* Error banner */}
      {error && (
        <div className="mx-4 mb-2 rounded-lg border border-red-300 bg-red-50 dark:bg-red-900/20 dark:border-red-700 px-3 py-2 text-sm text-red-700 dark:text-red-300 flex items-start justify-between gap-2">
          <span className="flex-1">{error}</span>
          <button
            onClick={clearError}
            className="text-red-500 hover:text-red-700 dark:hover:text-red-300 shrink-0"
          >
            ✕
          </button>
        </div>
      )}

      {/* Input */}
      <form
        onSubmit={handleSubmit}
        className="flex items-end gap-2 p-4 border-t border-zinc-200 dark:border-zinc-700"
      >
        <textarea
          ref={inputRef}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={t("chat.inputPlaceholder")}
          rows={1}
          className="flex-1 resize-none rounded-lg border border-zinc-300 dark:border-zinc-600 bg-white dark:bg-zinc-800 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
          disabled={isLoading}
        />
        <button
          type="submit"
          disabled={!input.trim() || isLoading}
          className="shrink-0 rounded-lg bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {isLoading ? "…" : t("chat.send")}
        </button>
      </form>
    </div>
  );
}