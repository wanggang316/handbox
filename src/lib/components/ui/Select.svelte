<script lang="ts">
  import { Select } from "bits-ui";
  import { Check, ChevronsUpDown } from "@lucide/svelte";
  import type { Snippet } from "svelte";
  import { getFormField } from "./FormField.svelte";
  import { cn } from "./utils";

  interface Option {
    value: string;
    label: string;
    disabled?: boolean;
  }

  interface Props {
    label?: string;
    value?: string;
    selectedValue?: string; // DropDown 兼容别名
    options?: Option[];
    placeholder?: string;
    autoWidth?: boolean;
    disabled?: boolean;
    /** standalone 错误态；在 FormField 内则由 FormField 接管。 */
    invalid?: boolean;
    size?: "sm" | "md" | "lg";
    class?: string;
    onChange?: (value: string) => void;
    onSelect?: (value: string, option: Option) => void; // DropDown 兼容
    /** 自定义 trailing 图标（默认 ChevronsUpDown）。 */
    icon?: Snippet;
  }

  let {
    label = "",
    value = $bindable(""),
    selectedValue = $bindable(),
    options = [],
    placeholder = "",
    autoWidth = false,
    disabled = false,
    invalid = false,
    size = "md",
    class: className = "",
    onChange = () => {},
    onSelect,
    icon,
  }: Props = $props();

  // selectedValue 优先（DropDown 兼容），否则用 value。
  const current = $derived(
    (selectedValue !== undefined ? selectedValue : value) ?? "",
  );
  const selectedLabel = $derived(
    options.find((o) => o.value === current)?.label ?? "",
  );

  function handleValueChange(v: string) {
    if (selectedValue !== undefined) selectedValue = v;
    else value = v;
    onChange(v);
    if (onSelect) {
      const opt = options.find((o) => o.value === v);
      if (opt) onSelect(v, opt);
    }
  }

  // FormField 内时接管 id / aria-invalid / describedby。
  const ff = getFormField();
  const fallbackId = `select-${Math.random().toString(36).slice(2, 11)}`;
  const controlId = $derived(ff ? ff.id : fallbackId);
  const isInvalid = $derived(ff ? ff.invalid : invalid);
  const describedby = $derived(ff ? ff.describedby : undefined);
  const showOwnLabel = $derived(!ff && !!label);

  const triggerSize = {
    sm: "h-7 px-2 text-xs",
    md: "h-8 px-3 text-sm",
    lg: "h-10 px-4 text-base",
  };
</script>

<div class={cn("inline-flex flex-col gap-1", autoWidth ? "" : "w-full", className)}>
  {#if showOwnLabel}
    <label for={controlId} class="text-sm font-medium text-base-content/80">
      {label}
    </label>
  {/if}

  <Select.Root
    type="single"
    value={current}
    onValueChange={handleValueChange}
    items={options}
    {disabled}
  >
    <Select.Trigger
      id={controlId}
      aria-invalid={isInvalid ? "true" : undefined}
      aria-describedby={describedby}
      class={cn(
        "inline-flex items-center justify-between gap-1.5 rounded-md border bg-transparent whitespace-nowrap text-base-content outline-none cursor-pointer transition-colors duration-[var(--dur-fast)] ease-[var(--ease-out)]",
        "border-[var(--field-border)] enabled:hover:border-[var(--field-border-hover)]",
        "focus-visible:ring-2 focus-visible:ring-primary/50 focus-visible:border-[var(--field-border-hover)]",
        "disabled:cursor-not-allowed disabled:opacity-50",
        "aria-invalid:border-error aria-invalid:ring-2 aria-invalid:ring-error/20",
        autoWidth ? "w-auto min-w-fit" : "w-full",
        triggerSize[size],
      )}
    >
      <span class={selectedLabel ? "truncate" : "truncate text-base-content/50"}>
        {selectedLabel || placeholder}
      </span>
      {#if icon}
        {@render icon()}
      {:else}
        <ChevronsUpDown
          size={size === "sm" ? 14 : 16}
          class="shrink-0 text-base-content/50"
        />
      {/if}
    </Select.Trigger>

    <Select.Portal>
      <Select.Content
        sideOffset={6}
        class="z-[var(--z-popover)] max-h-72 min-w-[var(--bits-floating-anchor-width)] overflow-y-auto rounded-md border border-[var(--hairline)] bg-[var(--bg-card)] shadow-lg outline-none"
      >
        <Select.Viewport class="p-1">
          {#each options as opt (opt.value)}
            <Select.Item
              value={opt.value}
              label={opt.label}
              disabled={opt.disabled}
              class="relative flex cursor-pointer select-none items-center rounded-md py-1.5 pl-2 pr-8 text-sm text-base-content outline-none transition-colors duration-[var(--dur-fast)] ease-[var(--ease-out)] data-highlighted:bg-base-300 data-disabled:pointer-events-none data-disabled:opacity-50"
            >
              {#snippet children({ selected })}
                <span class="truncate">{opt.label}</span>
                {#if selected}
                  <Check size={15} class="absolute right-2 text-primary" />
                {/if}
              {/snippet}
            </Select.Item>
          {/each}
        </Select.Viewport>
      </Select.Content>
    </Select.Portal>
  </Select.Root>
</div>
