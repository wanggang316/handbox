<script lang="ts">
    import DropDownRow from "$lib/components/ui/table/DropDownRow.svelte";
    import SwitchRow from "$lib/components/ui/table/SwitchRow.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";

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


<div class="p-6 pr-8 flex flex-col gap-y-4">

  <TableGroup>
    <DropDownRow label="发送消息" options={options} bind:selectedValue={selectedValue} onSelect={handleSelect} />
  </TableGroup>

</div>