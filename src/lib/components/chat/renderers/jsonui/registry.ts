/**
 * Runtime registry for the JSON-Render generative-UI PoC.
 *
 * Binds the four catalog component names to their `.svelte` implementations and
 * exports the registry consumed by `<Renderer spec registry={uiRegistry} />`.
 *
 * This is the only module in the directory that imports a `.svelte` file, so it
 * deliberately stays out of the Node-environment unit-test import graph (tests
 * target `resolveSpec.ts` + `catalog.ts`, which are pure TS).
 */

import { defineRegistry } from "@json-render/svelte";
import { uiCatalog } from "./catalog";
import Card from "./components/Card.svelte";
import Text from "./components/Text.svelte";
import Badge from "./components/Badge.svelte";
import Stack from "./components/Stack.svelte";

export const { registry: uiRegistry } = defineRegistry(uiCatalog, {
  components: { Card, Text, Badge, Stack },
});
