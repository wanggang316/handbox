<script lang="ts">
  import Button from "$lib/components/ui/Button.svelte";
  import Input from "$lib/components/ui/Input.svelte";
  import Textarea from "$lib/components/ui/Textarea.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import Checkbox from "$lib/components/ui/Checkbox.svelte";
  import RadioGroup from "$lib/components/ui/RadioGroup.svelte";
  import FormField from "$lib/components/ui/FormField.svelte";
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import RoundButton from "$lib/components/ui/RoundButton.svelte";
  import ArrowButton from "$lib/components/ui/ArrowButton.svelte";
  import TrafficLightsRedButton from "$lib/components/ui/TrafficLightsRedButton.svelte";
  import { Settings, Plus, Check, Search } from "@lucide/svelte";

  let name = $state("");
  let email = $state("bad-value");
  let bio = $state("");
  let provider = $state("openai");
  let agree = $state(true);
  let partial = $state(true);
  let plan = $state("pro");

  const providerOptions = [
    { value: "openai", label: "OpenAI" },
    { value: "anthropic", label: "Anthropic" },
    { value: "google", label: "Google AI" },
  ];
  const planOptions = [
    { value: "free", label: "Free" },
    { value: "pro", label: "Pro" },
    { value: "team", label: "Team（暂不可用）", disabled: true },
  ];
</script>

<div class="max-w-2xl space-y-8 p-6 pt-2">
  <header class="space-y-1">
    <p class="text-sm text-base-content/70">
      标准化表单控件预览——统一 --field-* token 与 .field 皮肤，明暗主题一致。
    </p>
  </header>

  <!-- 表单字段：FormField 统一 label / required / error / hint，控件共用 .field 皮肤 -->
  <section class="space-y-4">
    <h2 class="text-base font-medium text-base-content">表单字段</h2>
    <div class="space-y-4">
      <FormField
        label="名称"
        required
        hint="展示 label / required / hint 的统一布局。"
      >
        <Input bind:value={name} placeholder="输入名称" />
      </FormField>

      <FormField label="邮箱" error="邮箱格式不正确。">
        <Input
          bind:value={email}
          placeholder="error 态：ring 与文字统一走 --field-error"
        />
      </FormField>

      <FormField label="供应商">
        <Select bind:value={provider} options={providerOptions} />
      </FormField>

      <FormField label="简介" hint="Textarea 与 Input 同一套皮肤。">
        <Textarea bind:value={bio} rows={3} placeholder="输入简介" />
      </FormField>

      <FormField label="禁用态">
        <Input value="disabled" disabled />
      </FormField>
    </div>
  </section>

  <!-- 选择控件：Checkbox / RadioGroup（bits-ui 行为 + token 皮肤） -->
  <section class="space-y-4">
    <h2 class="text-base font-medium text-base-content">选择控件</h2>
    <div class="space-y-4">
      <div class="flex flex-col gap-2">
        <Checkbox bind:checked={agree}>已同意条款</Checkbox>
        <Checkbox bind:indeterminate={partial}>部分选中（indeterminate）</Checkbox>
        <Checkbox checked disabled>已选中 · 禁用</Checkbox>
      </div>

      <FormField label="套餐">
        <RadioGroup bind:value={plan} options={planOptions} />
      </FormField>

      <FormField label="套餐（横向）">
        <RadioGroup
          bind:value={plan}
          options={planOptions}
          orientation="horizontal"
        />
      </FormField>
    </div>
  </section>

  <!-- 按钮：全家族 variant × size × state 矩阵，作回归验收基线 -->
  <section class="space-y-4">
    <h2 class="text-base font-medium text-base-content">按钮</h2>

    <div class="space-y-2">
      <p class="text-xs text-base-content/60">Button · variant</p>
      <div class="flex flex-wrap items-center gap-2">
        <Button variant="primary">Primary</Button>
        <Button variant="secondary">Secondary</Button>
        <Button variant="gray">Gray</Button>
        <Button variant="ghost">Ghost</Button>
        <Button variant="clear">Clear</Button>
        <Button variant="danger">Danger</Button>
      </div>
    </div>

    <div class="space-y-2">
      <p class="text-xs text-base-content/60">Button · size / state</p>
      <div class="flex flex-wrap items-center gap-2">
        <Button size="sm">Small</Button>
        <Button size="md">Medium</Button>
        <Button disabled>Disabled</Button>
      </div>
    </div>

    <div class="space-y-2">
      <p class="text-xs text-base-content/60">
        图标 / 形状按钮 · IconButton · CircleButton · ArrowButton · TrafficLight
      </p>
      <div class="flex flex-wrap items-center gap-3">
        <IconButton icon={Settings} ariaLabel="设置" />
        <IconButton icon={Search} ariaLabel="搜索" />
        <IconButton icon={Plus} ariaLabel="新增（禁用）" disabled />
        <CircleButton icon={Plus} ariaLabel="新增" variant="neutral" />
        <CircleButton icon={Check} ariaLabel="确认" variant="secondary" />
        <ArrowButton label="展开" />
        <TrafficLightsRedButton />
      </div>
    </div>

    <div class="space-y-2">
      <p class="text-xs text-base-content/60">RoundButton · variant / loading</p>
      <div class="flex flex-wrap items-center gap-2">
        <RoundButton label="Primary" variant="primary" />
        <RoundButton label="Accent" variant="accent" />
        <RoundButton label="Danger" variant="danger" />
        <RoundButton label="Secondary" variant="secondary" />
        <RoundButton label="Loading" variant="primary" loading />
      </div>
    </div>
  </section>
</div>
