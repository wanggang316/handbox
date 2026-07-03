<script lang="ts">
  import type { Snippet } from "svelte";
  import { ChevronsUpDown } from "@lucide/svelte";
  import { getFormField } from "./FormField.svelte";

  interface Option {
    value: string;
    label: string;
  }

  interface Props {
    label?: string;
    value?: string | number;
    selectedValue?: string | number; // DropDown 兼容性
    options?: Option[];
    placeholder?: string;
    autoWidth?: boolean;
    disabled?: boolean;
    size?: "sm" | "md" | "lg";
    class?: string;
    onChange?: (value: string) => void;
    onSelect?: (value: string, option: Option) => void; // DropDown 兼容性
    showIcon?: boolean; // 是否显示下拉箭头，默认 true
    icon?: Snippet;
    children?: Snippet;
  }

  let {
    label = "",
    value = $bindable(""),
    selectedValue = $bindable(), // DropDown 兼容性
    options = [],
    placeholder = "",
    autoWidth = false,
    disabled = false,
    size = "md",
    class: className = "",
    onChange = () => {},
    onSelect,
    showIcon = true,
    icon,
    children,
  }: Props = $props();

  // 兼容 DropDown 的 selectedValue prop
  const internalValue = $derived(selectedValue !== undefined ? selectedValue : value);

  const sizeClasses = {
    sm: "px-2 py-1 text-xs",
    md: "px-3 py-2 text-sm",
    lg: "px-4 py-3 text-base",
  };

  // 图标大小
  const iconSizes = {
    sm: 14,
    md: 16,
    lg: 18,
  };

  // 是否显示图标（自定义图标或默认图标）
  const hasIcon = $derived(showIcon || icon);

  const id = `select-${Math.random().toString(36).slice(2, 11)}`;

  // FormField 内时接管 id / aria / error 态，并抑制自渲 label（容器统一提供）。
  const ff = getFormField();
  const controlId = $derived(ff ? ff.id : id);
  const invalid = $derived(ff ? ff.invalid : false);
  const describedby = $derived(ff ? ff.describedby : undefined);
  const showOwnLabel = $derived(!ff && !!label);

  function handleChange(e: Event) {
    const target = e.currentTarget as HTMLSelectElement;
    const newValue = target.value;

    // 更新 value
    if (selectedValue !== undefined) {
      selectedValue = newValue;
    } else {
      value = newValue;
    }

    // 调用回调
    onChange(newValue);

    // DropDown 兼容：调用 onSelect
    if (onSelect) {
      const option = options.find((opt) => opt.value === newValue);
      if (option) {
        onSelect(newValue, option);
      }
    }
  }
</script>

<div class="inline-flex flex-col gap-1 {className}">
  {#if showOwnLabel}
    <label for={controlId} class="text-sm font-medium text-base-content/80">
      {label}
    </label>
  {/if}

  <div class="relative {autoWidth ? 'inline-flex' : 'w-full'}">
    <select
      id={controlId}
      value={internalValue}
      {disabled}
      onchange={handleChange}
      aria-invalid={invalid ? "true" : undefined}
      aria-describedby={describedby}
      class:is-error={invalid}
      class="field field--soft appearance-none {autoWidth
        ? 'w-auto min-w-fit'
        : 'w-full'} {sizeClasses[size]} {hasIcon
        ? size === 'sm'
          ? 'pr-6'
          : 'pr-8'
        : size === 'sm'
          ? 'pr-2'
          : 'pr-3'} cursor-pointer"
    >
      {#if placeholder}
        <option value="" disabled selected>{placeholder}</option>
      {/if}

      {#each options as opt (opt.value)}
        <option value={opt.value}>{opt.label}</option>
      {/each}

      {#if children}
        {@render children()}
      {/if}
    </select>

    {#if hasIcon}
      <div
        class="absolute right-2 top-1/2 -translate-y-1/2 text-base-content/60 pointer-events-none"
      >
        {#if icon}
          {@render icon()}
        {:else}
          <ChevronsUpDown size={iconSizes[size]} />
        {/if}
      </div>
    {/if}
  </div>
</div>
