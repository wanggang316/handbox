<script lang="ts">
  import SelectRow from "$lib/components/ui/table/SelectRow.svelte";
  import SwitchRow from "$lib/components/ui/table/SwitchRow.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import { t } from "$lib/i18n";

  const options = [
    { value: "1", label: "Enter" },
    { value: "2", label: "Command+Enter" },
  ];

  let selectedValue = $state("1");

  function handleSelect(value: string) {
    selectedValue = value;
    console.log(selectedValue);
  }
</script>

<div class="mt-8 p-6 pr-8 flex flex-col gap-y-4">
  <TableGroup>
    <SelectRow
      label={t("settings.shortcuts.sendMessage")}
      {options}
      bind:selectedValue
      onSelect={handleSelect}
    />
  </TableGroup>
</div>
