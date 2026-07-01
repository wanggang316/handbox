<script lang="ts">
  import type { BaseComponentProps } from "@json-render/svelte";

  // A read-only table: a header row from `columns` plus one data row per `rows`
  // entry. Every cell renders through Svelte text binding only (never @html).
  // Cells are always laid out by column index, so a row with fewer cells than
  // columns leaves the trailing cells blank rather than shifting alignment, and a
  // zero-row table still renders its header without collapsing.
  interface TableProps {
    columns: string[];
    rows: string[][];
  }

  let { props }: BaseComponentProps<TableProps> = $props();
</script>

<div
  class="overflow-hidden rounded-lg border border-[var(--hairline)] text-sm"
>
  <table class="w-full border-collapse">
    <thead>
      <tr>
        {#each props.columns as column, i (i)}
          <th
            class="border-b border-[var(--hairline)] px-3 py-2 text-left font-medium text-base-content/70 break-words"
          >
            {column}
          </th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each props.rows as row, r (r)}
        <tr>
          {#each props.columns as _column, c (c)}
            <td
              class="border-t border-[var(--hairline)] px-3 py-2 text-base-content break-words"
            >
              {row[c] ?? ""}
            </td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
</div>
