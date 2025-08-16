<script lang="ts">
  import { ChevronDown } from '@lucide/svelte';

  interface DropDownOption {
    value: string;
    label: string;
    disabled?: boolean;
  }

  interface Props {
    options: DropDownOption[];
    selectedValue?: string;
    placeholder?: string;
    disabled?: boolean;
    position?: 'bottom' | 'top';
    align?: 'left' | 'right';
    minWidth?: string;
    maxHeight?: string;
    buttonClass?: string;
    dropdownClass?: string;
    optionClass?: string;
    selectedOptionClass?: string;
    onSelect?: (value: string, option: DropDownOption) => void;
  }

  let {
    options = [],
    selectedValue = $bindable(''),
    placeholder = '请选择...',
    disabled = false,
    position = 'bottom',
    align = 'left',
    minWidth = 'min-w-40',
    maxHeight = 'max-h-64',
    buttonClass = '',
    dropdownClass = '',
    optionClass = '',
    selectedOptionClass = '',
    onSelect = (value: string, option: DropDownOption) => {}
  }: Props = $props();

  let isOpen = $state(false);
  let dropdownRef: HTMLDivElement;

  // 获取选中选项的标签
  const selectedLabel = $derived.by(() => {
    const selected = options.find(option => option.value === selectedValue);
    return selected ? selected.label : placeholder;
  });

  // 切换下拉菜单显示状态
  function toggle() {
    if (disabled) return;
    isOpen = !isOpen;
  }

  // 选择选项
  function selectOption(option: DropDownOption) {
    if (option.disabled) return;
    
    selectedValue = option.value;
    isOpen = false;
    onSelect(option.value, option);
  }

  // 处理键盘事件
  function handleKeydown(event: KeyboardEvent) {
    if (disabled) return;
    
    switch (event.key) {
      case 'Enter':
      case ' ':
        event.preventDefault();
        toggle();
        break;
      case 'Escape':
        isOpen = false;
        break;
      case 'ArrowDown':
        event.preventDefault();
        if (!isOpen) {
          isOpen = true;
        } else {
          // 可以添加键盘导航逻辑
        }
        break;
      case 'ArrowUp':
        event.preventDefault();
        // 可以添加键盘导航逻辑
        break;
    }
  }

  // 点击外部关闭下拉菜单
  function handleClickOutside(event: MouseEvent) {
    if (dropdownRef && !dropdownRef.contains(event.target as Node)) {
      isOpen = false;
    }
  }

  // 组合样式类
  const defaultButtonClass = "h-8 px-3 rounded-md text-[14px] leading-[1.2] text-black flex items-center gap-1 hover:bg-bg-hover transition-colors";
  const finalButtonClass = `${defaultButtonClass} ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'} ${buttonClass}`;

  const positionClass = position === 'top' ? 'bottom-full mb-1' : 'top-full mt-1';
  const alignClass = align === 'right' ? 'right-0' : 'left-0';
  const defaultDropdownClass = `absolute ${positionClass} ${alignClass} ${minWidth} ${maxHeight} bg-white border border-[#e5e5e5] rounded-lg shadow-md z-10 overflow-y-auto`;
  const finalDropdownClass = `${defaultDropdownClass} ${dropdownClass}`;

  const defaultOptionClass = "w-full px-3 py-2 text-left text-[14px] hover:bg-gray-50 transition-colors";
  const defaultSelectedOptionClass = "bg-blue-50 text-blue-600 font-medium";
</script>

<svelte:window on:click={handleClickOutside} />

<div class="relative inline-block" bind:this={dropdownRef}>
  <button
    type="button"
    class={finalButtonClass}
    onclick={toggle}
    onkeydown={handleKeydown}
    aria-haspopup="listbox"
    aria-expanded={isOpen}
    aria-label="下拉选择"
    {disabled}
  >
    <span class="truncate flex-1 text-left">{selectedLabel}</span>
    <ChevronDown 
      size={16} 
      class="text-gray-500 transition-transform {isOpen ? 'rotate-180' : ''}"
    />
  </button>

  {#if isOpen}
    <div class={finalDropdownClass} role="listbox">
      {#each options as option}
        <button
          type="button"
          class="{defaultOptionClass} {option.value === selectedValue ? defaultSelectedOptionClass + ' ' + selectedOptionClass : ''} {option.disabled ? 'opacity-50 cursor-not-allowed' : ''} {optionClass}"
          onclick={() => selectOption(option)}
          disabled={option.disabled}
          role="option"
          aria-selected={option.value === selectedValue}
        >
          {option.label}
        </button>
      {/each}
      
      {#if options.length === 0}
        <div class="px-3 py-2 text-[14px] text-gray-500">
          暂无选项
        </div>
      {/if}
    </div>
  {/if}
</div>
