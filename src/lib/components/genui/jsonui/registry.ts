/**
 * Binds the catalog component names to their `.svelte` implementations for
 * `<Renderer spec registry={uiRegistry} />`. The only module in this directory
 * that imports a `.svelte` file, so it deliberately stays out of the
 * Node-environment unit-test import graph.
 */

import { defineRegistry } from "@json-render/svelte";
import { uiCatalog } from "./catalog";
import Card from "./components/Card.svelte";
import Text from "./components/Text.svelte";
import Badge from "./components/Badge.svelte";
import Stack from "./components/Stack.svelte";
import StatusLabel from "./components/StatusLabel.svelte";
import Avatar from "./components/Avatar.svelte";
import Divider from "./components/Divider.svelte";
import KeyValue from "./components/KeyValue.svelte";
import Table from "./components/Table.svelte";
import InfoTooltip from "./components/InfoTooltip.svelte";

export const { registry: uiRegistry } = defineRegistry(uiCatalog, {
  components: {
    Card,
    Text,
    Badge,
    Stack,
    StatusLabel,
    Avatar,
    Divider,
    KeyValue,
    Table,
    InfoTooltip,
  },
});
