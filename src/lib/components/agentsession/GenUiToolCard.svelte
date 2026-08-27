<script module lang="ts">
  /**
   * Spec text by GenUI id, and the in-flight fetches that fill it. Module-level
   * so a transcript that calls the same tool ten times fetches once, and so
   * revisiting a session repaints from memory instead of a round trip.
   *
   * A failed fetch caches nothing: the card falls back to the plain tool row and
   * the next mount tries again.
   */
  const specCache = new Map<string, string>();
  const specInFlight = new Map<string, Promise<void>>();

  async function loadSpec(genuiId: string): Promise<void> {
    if (specCache.has(genuiId)) return;
    let pending = specInFlight.get(genuiId);
    if (!pending) {
      pending = getGenui(genuiId)
        .then((genui) => {
          specCache.set(genuiId, genui.spec);
        })
        .catch((error) => {
          console.error("Failed to load the tool's view:", error);
        })
        .finally(() => {
          specInFlight.delete(genuiId);
        });
      specInFlight.set(genuiId, pending);
    }
    return pending;
  }
</script>

<script lang="ts">
  import { Renderer, JsonUIProvider } from "@json-render/svelte";
  import { getGenui } from "$lib/api/genui";
  import { uiRegistry } from "$lib/components/genui/jsonui/registry";
  import { resolveSpec } from "$lib/components/genui/jsonui/resolveSpec";
  import { toolArgsRecord } from "$lib/utils/toolCall";
  import type { ToolCallView } from "$lib/states/agentRun.svelte";
  import type { ToolDefinition } from "$lib/types/toolDefinition";
  import AgentToolCallCard from "./AgentToolCallCard.svelte";

  interface Props {
    toolCall: ToolCallView;
    /** The definition behind this call; the caller has checked it has a view. */
    definition: ToolDefinition;
  }

  let { toolCall, definition }: Props = $props();

  // Repaint when the fetch fills the cache. The cache itself is a plain Map (it
  // outlives every component), so this local mirror is what carries reactivity.
  let specText = $state<string | null>(null);

  $effect(() => {
    const genuiId = definition.genuiId;
    if (!genuiId) return;
    specText = specCache.get(genuiId) ?? null;
    if (specText !== null) return;
    void loadSpec(genuiId).then(() => {
      specText = specCache.get(genuiId) ?? null;
    });
  });

  // The author's spec binds props to the call's arguments, so it is validated
  // with bindings allowed — unlike a model's reply, which must be literal.
  const spec = $derived(
    specText === null ? null : resolveSpec(specText, { allowBindings: true }),
  );

  // The arguments ARE the state model: `{ $state: "/city" }` in the spec reads
  // the `city` argument of this call. Deliberately not named `state` — that
  // would make every `$state` rune below parse as a store subscription.
  const argumentState = $derived(toolArgsRecord(toolCall.args));

  // An in-flight fetch, a deleted GenUI, or a spec that no longer validates all
  // land here. A tool call is part of the transcript either way, so it falls
  // back to the ordinary row rather than to a gap.
  const renderable = $derived(spec !== null && toolCall.status !== "error");
</script>

{#if renderable && spec}
  <!-- A view is user-authored and renders arguments a model produced, so it is
       the one place in the timeline where a render error is plausible. The
       boundary keeps that failure inside this one card instead of taking the
       message stream down with it. -->
  <svelte:boundary>
    <JsonUIProvider initialState={argumentState}>
      <Renderer {spec} registry={uiRegistry} />
    </JsonUIProvider>

    {#snippet failed()}
      <AgentToolCallCard {toolCall} />
    {/snippet}
  </svelte:boundary>
{:else}
  <AgentToolCallCard {toolCall} />
{/if}
