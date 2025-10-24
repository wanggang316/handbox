<script lang="ts">
  interface Props {
    label?: string;
    checked?: boolean;
    onChange?: (v: boolean) => void;
    onChangeBefore?: (next: boolean, previous: boolean) => boolean | Promise<boolean>;
    id?: string;
    disabled?: boolean;
  }

  let {
    label = "",
    checked = $bindable(false),
    onChange = () => {},
    onChangeBefore = () => true,
    id = `tgl-${Math.random().toString(36).slice(2)}`,
    disabled = false,
  }: Props = $props();

  async function handleChange(event: Event) {
    if (disabled) return;

    const target = event.currentTarget as HTMLInputElement;
    const next = target.checked;
    const previous = checked;

    let allow = true;
    if (onChangeBefore) {
      try {
        allow = await onChangeBefore(next, previous);
      } catch (error) {
        console.error("Toggle onChangeBefore failed:", error);
        allow = false;
      }
    }

    if (!allow) {
      target.checked = previous;
      checked = previous;
      return;
    }

    checked = next;
    onChange(next);
  }
</script>

<label class="toggle" class:disabled>
  <input
    {id}
    type="checkbox"
    checked={checked}
    {disabled}
    onchange={handleChange}
  />
  <span class="slider" aria-hidden="true"></span>
  {#if label}
    <span class="text">{label}</span>
  {/if}
</label>

<style>
  .toggle {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
  }
  .toggle.disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .toggle input {
    display: none;
  }
  .slider {
    width: 36px;
    height: 20px;
    border-radius: 999px;
    background: var(--base-300);
    position: relative;
    transition: all 0.2s;
  }
  .slider::after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--base-100);
    transition: transform 0.2s;
    box-shadow: 0 1px 2px
      color-mix(in oklch, var(--base-content) 20%, transparent);
  }
  input:checked + .slider {
    background: var(--primary);
  }
  input:checked + .slider::after {
    transform: translateX(16px);
  }
  .text {
    color: var(--base-content);
  }
</style>
