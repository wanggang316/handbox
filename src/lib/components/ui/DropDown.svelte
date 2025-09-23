<script lang="ts">
  import { ChevronDown, ChevronsUpDown } from '@lucide/svelte';

  interface DropDownOption {
    value: string;
    label: string;
    disabled?: boolean;
  }

  interface DropDownGroup {
    title?: string;
    options: DropDownOption[];
  }

  interface Props {
    options?: DropDownOption[];
    groups?: DropDownGroup[];
    selectedValue?: string;
    placeholder?: string;
    disabled?: boolean;
    position?: 'bottom' | 'top';
    align?: 'left' | 'right';
    minWidth?: string;
    maxWidth?: string;
    maxHeight?: string;
    buttonClass?: string;
    dropdownClass?: string;
    optionClass?: string;
    selectedOptionClass?: string;
    groupTitleClass?: string;
    separatorClass?: string;
    onSelect?: (value: string, option: DropDownOption) => void;
  }

  let {
    options = [],
    groups = [],
    selectedValue = $bindable(''),
    placeholder = '请选择...',
    disabled = false,
    position = 'bottom',
    align = 'left',
    minWidth = 'min-w-20',
    maxWidth = 'max-w-80',
    maxHeight = 'max-h-64',
    buttonClass = '',
    dropdownClass = '',
    optionClass = '',
    selectedOptionClass = '',
    groupTitleClass = '',
    separatorClass = '',
    onSelect = (_value: string, _option: DropDownOption) => {}
  }: Props = $props();

  let isOpen = $state(false);
  let dropdownRef: HTMLDivElement;

  // 获取所有选项（合并 options 和 groups 中的选项）
  const allOptions = $derived.by(() => {
    const flatOptions = [...options];
    groups.forEach(group => {
      flatOptions.push(...group.options);
    });
    return flatOptions;
  });

  // 获取选中选项的标签
  const selectedLabel = $derived.by(() => {
    const selected = allOptions.find(option => option.value === selectedValue);
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
  const defaultButtonClass = "h-8 px-2 rounded-md text-[14px] leading-[1.2] text-base-content flex items-center gap-1 hover:bg-base-300";
  const finalButtonClass = `${defaultButtonClass} ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'} ${buttonClass}`;

  const positionClass = position === 'top' ? 'bottom-full mb-1' : 'top-full mt-1';
  const alignClass = align === 'right' ? 'right-0' : 'left-0';
  const defaultDropdownClass = `absolute ${positionClass} ${alignClass} ${minWidth} ${maxWidth} ${maxHeight} bg-base-100 border border-base-300 rounded-xl shadow-xl z-[10020] overflow-y-auto w-fit px-1 py-1`;
  const finalDropdownClass = `${defaultDropdownClass} ${dropdownClass}`;

  const defaultOptionClass = "w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 whitespace-nowrap";
  const defaultSelectedOptionClass = "";
  const defaultGroupTitleClass = "px-2 py-1 text-[11px] text-base-content/70";
  const defaultSeparatorClass = "border-t border-base-300 my-1 mx-2";


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
    <ChevronsUpDown 
      size={16} 
      class="text-base-content/60"
    />
  </button>

  {#if isOpen}
    <div class={finalDropdownClass} role="listbox">
      <!-- 渲染单独的 options -->
      {#if options.length > 0}
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
        
        <!-- 如果同时有 groups，添加分割线 -->
        {#if groups.length > 0}
          <div class="{defaultSeparatorClass} {separatorClass}"></div>
        {/if}
      {/if}
      
      <!-- 渲染分组 options -->
      {#each groups as group, groupIndex}
        {#if group.title}
          <div class="{defaultGroupTitleClass} {groupTitleClass}" role="group" aria-label={group.title}>
            {group.title}
          </div>
        {/if}
        
        {#each group.options as option}
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
        
        <!-- 在组之间添加分割线（除了最后一组） -->
        {#if groupIndex < groups.length - 1}
          <div class="{defaultSeparatorClass} {separatorClass}"></div>
        {/if}
      {/each}
      
      <!-- 没有任何选项时显示提示 -->
      {#if options.length === 0 && groups.length === 0}
        <div class="px-3 py-2 text-[14px] text-base-content/70">
          暂无选项
        </div>
      {/if}
      
      <!-- 所有组都没有选项时显示提示 -->
      {#if options.length === 0 && groups.length > 0 && groups.every(g => g.options.length === 0)}
        <div class="px-3 py-2 text-[14px] text-base-content/70">
          暂无选项
        </div>
      {/if}
    </div>
  {/if}
</div>
