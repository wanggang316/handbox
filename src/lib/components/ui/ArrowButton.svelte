<script lang="ts">
  import { ChevronsUpDown } from "@lucide/svelte";
  import type { Component } from "svelte";

  interface Props {
    label: string;
    onclick?: () => void;
    icon?: Component;
    iconSize?: number;
  }

  let {
    label,
    onclick,
    icon: Icon = ChevronsUpDown,
    iconSize = 12,
  }: Props = $props();
</script>

<button
  type="button"
  class="flex items-center gap-1 text-xs text-base-content/60 hover:text-base-content hover:bg-base-200 rounded py-0.5 transition-colors"
  {onclick}
>
  <span>{label}</span>
  <Icon size={iconSize} />
</button>
