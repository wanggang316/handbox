<script lang="ts">
  import Button from "$lib/components/ui/Button.svelte";
  import Input from "$lib/components/ui/Input.svelte";
  import Textarea from "$lib/components/ui/Textarea.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import Checkbox from "$lib/components/ui/Checkbox.svelte";
  import RadioGroup from "$lib/components/ui/RadioGroup.svelte";
  import FormField from "$lib/components/ui/FormField.svelte";

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

<div class="max-w-2xl space-y-8 p-6">
  <header class="space-y-1">
    <h1 class="text-xl font-medium text-base-content">UI 组件</h1>
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

  <!-- 按钮 -->
  <section class="space-y-4">
    <h2 class="text-base font-medium text-base-content">按钮</h2>
    <div class="flex flex-wrap gap-2">
      <Button variant="primary">Primary</Button>
      <Button variant="secondary">Secondary</Button>
      <Button variant="gray">Gray</Button>
      <Button variant="danger">Danger</Button>
      <Button disabled>Disabled</Button>
    </div>
  </section>
</div>
