<script lang="ts">
  import { Bell, Box, LayoutGrid, Settings, User } from "@lucide/svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import RoundButton from "$lib/components/ui/RoundButton.svelte";
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import ArrowButton from "$lib/components/ui/ArrowButton.svelte";
  import TrafficLightsRedButton from "$lib/components/ui/TrafficLightsRedButton.svelte";
  import Input from "$lib/components/ui/Input.svelte";
  import Textarea from "$lib/components/ui/Textarea.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import Slider from "$lib/components/ui/Slider.svelte";
  import LabeledSlider from "$lib/components/ui/LabeledSlider.svelte";
  import NumberStepper from "$lib/components/ui/NumberStepper.svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import ConfirmModal from "$lib/components/ui/ConfirmModal.svelte";
  import Drawer from "$lib/components/ui/Drawer.svelte";
  import InfoTooltip from "$lib/components/ui/InfoTooltip.svelte";
  import Tabs from "$lib/components/ui/Tabs.svelte";
  import StatusLabel from "$lib/components/ui/StatusLabel.svelte";
  import Avatar from "$lib/components/ui/Avatar.svelte";
  import Menu from "$lib/components/ui/Menu.svelte";
  import ResizableSidebar from "$lib/components/ui/ResizableSidebar.svelte";
  import ChatList from "$lib/components/ui/ChatList.svelte";
  import DefaultRow from "$lib/components/ui/table/DefaultRow.svelte";
  import {
    TableGroup,
    SwitchRow,
    SelectRow,
    NumberStepperRow,
    LabeledSliderRow,
    TextareaRow,
    TextRow,
    StatusLabelRow
  } from "$lib/components/ui/table";
  import { toastActions } from "$lib/states/toast.svelte";

  let textValue = $state("Hello Handbox");
  let textareaValue = $state("这是一个多行输入示例。\n支持换行与字符计数。");
  let selectValue = $state("beta");
  let toggleValue = $state(true);
  let sliderValue = $state(35);
  let labeledValue = $state(0.6);
  let numberValue = $state(3);
  let tabValue = $state("overview");

  let modalOpen = $state(false);
  let confirmOpen = $state(false);
  let drawerOpen = $state(false);

  let tableToggle = $state(true);
  let tableSelect = $state("alpha");
  let tableNumber = $state(2);
  let tableTextarea = $state("配置说明，支持多行内容。");
  let tableText = $state("可编辑值");

  let activeMenuId = $state("profile");

  const selectOptions = [
    { value: "alpha", label: "Alpha" },
    { value: "beta", label: "Beta" },
    { value: "gamma", label: "Gamma" }
  ];

  const tabItems = [
    { value: "overview", label: "概览" },
    { value: "details", label: "详情" },
    { value: "activity", label: "动态" }
  ];

  const menuItems = [
    { id: "profile", title: "个人资料", icon: User },
    { id: "notifications", title: "消息通知", icon: Bell },
    { id: "preferences", title: "偏好设置", icon: Settings },
    { id: "workspace", title: "工作区", icon: LayoutGrid }
  ];

  const chatSamples = [
    { id: "chat-1", title: "产品定位讨论" },
    { id: "chat-2", title: "模型表现评估" },
    { id: "chat-3", title: "组件 API 设计" }
  ];

  function triggerToast(type: "success" | "info" | "warning" | "error") {
    const messages = {
      success: "保存成功",
      info: "信息已更新",
      warning: "请检查输入",
      error: "操作失败"
    } as const;

    toastActions[type](messages[type], {
      hint: "这是一个示例 Toast",
      code: type.toUpperCase()
    });
  }
</script>

<div class="p-6 pr-8 space-y-10">
  <header class="space-y-2">
    <h1 class="text-xl font-semibold text-base-content">UI 组件测试</h1>
    <p class="text-sm text-base-content/70">
      用于集中预览公共组件的状态与交互，按类型划分展示。
    </p>
  </header>

  <section class="space-y-4">
    <h2 class="text-base font-semibold text-base-content">按钮类</h2>
    <div class="grid gap-4 lg:grid-cols-2">
      <div class="rounded-xl border border-base-300 bg-base-100 p-4 space-y-3">
        <div class="text-xs text-base-content/60">Button</div>
        <div class="flex flex-wrap items-center gap-2">
          <Button>Primary</Button>
          <Button variant="secondary">Secondary</Button>
          <Button variant="gray">Gray</Button>
          <Button variant="ghost">Ghost</Button>
          <Button variant="danger">Danger</Button>
          <Button variant="clear">Clear</Button>
        </div>
      </div>

      <div class="rounded-xl border border-base-300 bg-base-100 p-4 space-y-3">
        <div class="text-xs text-base-content/60">RoundButton / CircleButton / IconButton</div>
        <div class="flex flex-wrap items-center gap-3">
          <RoundButton label="确认" />
          <RoundButton label="加载中" loading />
          <CircleButton icon={Box} ariaLabel="Circle" />
          <IconButton icon={Settings} ariaLabel="Settings" />
          <TrafficLightsRedButton />
        </div>
      </div>

      <div class="rounded-xl border border-base-300 bg-base-100 p-4 space-y-3">
        <div class="text-xs text-base-content/60">ArrowButton</div>
        <ArrowButton label="高级选项" />
      </div>
    </div>
  </section>

  <section class="space-y-4">
    <h2 class="text-base font-semibold text-base-content">表单类</h2>
    <div class="grid gap-4 lg:grid-cols-2">
      <div class="rounded-xl border border-base-300 bg-base-100 p-4 space-y-3">
        <div class="text-xs text-base-content/60">Input / Select</div>
        <Input
          label="名称"
          placeholder="请输入名称"
          value={textValue}
          onInput={(val) => (textValue = val)}
        />
        <Select
          label="状态"
          options={selectOptions}
          placeholder="请选择"
          bind:selectedValue={selectValue}
          onChange={(value) => (selectValue = value)}
        />
      </div>

      <div class="rounded-xl border border-base-300 bg-base-100 p-4 space-y-3">
        <div class="text-xs text-base-content/60">Textarea</div>
        <Textarea
          bind:value={textareaValue}
          rows={4}
          maxlength={120}
          showCharCount
        />
      </div>

      <div class="rounded-xl border border-base-300 bg-base-100 p-4 space-y-3">
        <div class="text-xs text-base-content/60">Toggle / NumberStepper</div>
        <div class="flex items-center gap-4">
          <Toggle label="启用" bind:checked={toggleValue} />
          <NumberStepper bind:value={numberValue} min={0} max={10} step={1} />
        </div>
      </div>

      <div class="rounded-xl border border-base-300 bg-base-100 p-4 space-y-3">
        <div class="text-xs text-base-content/60">Slider / LabeledSlider</div>
        <Slider
          label="紧凑度"
          bind:value={sliderValue}
          min={0}
          max={100}
          step={5}
          description="拖动滑杆调整参数"
        />
        <LabeledSlider
          bind:value={labeledValue}
          min={0}
          max={1}
          step={0.1}
          leftLabel="保守"
          rightLabel="激进"
          showScaleMarks
          scaleMarks={[
            { value: 0, position: 0 },
            { value: 0.5, position: 50 },
            { value: 1, position: 100 }
          ]}
        />
      </div>
    </div>
  </section>

  <section class="space-y-4">
    <h2 class="text-base font-semibold text-base-content">导航与布局</h2>
    <div class="grid gap-4 lg:grid-cols-2">
      <div class="rounded-xl border border-base-300 bg-base-100 p-4 space-y-3">
        <div class="text-xs text-base-content/60">Tabs</div>
        <Tabs value={tabValue} items={tabItems} onChange={(val) => (tabValue = val)} />
      </div>

      <div class="rounded-xl border border-base-300 bg-base-100 p-4 space-y-3">
        <div class="text-xs text-base-content/60">Menu</div>
        <Menu
          items={menuItems}
          activeId={activeMenuId}
          onItemClick={(item) => (activeMenuId = item.id)}
        />
      </div>

      <div class="rounded-xl border border-base-300 bg-base-100 p-4 space-y-3">
        <div class="text-xs text-base-content/60">ResizableSidebar</div>
        <div class="flex h-40 rounded-lg border border-base-300 overflow-hidden">
          <ResizableSidebar
            initialWidth={180}
            minWidth={140}
            maxWidth={240}
            storageKey="components.sidebar.demo"
          >
            <div class="h-full bg-base-200 p-3 text-xs text-base-content/70">
              可拖拽侧栏
            </div>
          </ResizableSidebar>
          <div class="flex-1 p-3 text-xs text-base-content/70">
            主区域内容
          </div>
        </div>
      </div>
    </div>
  </section>

  <section class="space-y-4">
    <h2 class="text-base font-semibold text-base-content">反馈与弹层</h2>
    <div class="grid gap-4 lg:grid-cols-2">
      <div class="rounded-xl border border-base-300 bg-base-100 p-4 space-y-3">
        <div class="text-xs text-base-content/60">Toast</div>
        <div class="flex flex-wrap gap-2">
          <Button size="sm" on:click={() => triggerToast("success")}>Success</Button>
          <Button size="sm" variant="secondary" on:click={() => triggerToast("info")}>Info</Button>
          <Button size="sm" variant="gray" on:click={() => triggerToast("warning")}>Warning</Button>
          <Button size="sm" variant="danger" on:click={() => triggerToast("error")}>Error</Button>
        </div>
      </div>

      <div class="rounded-xl border border-base-300 bg-base-100 p-4 space-y-3">
        <div class="text-xs text-base-content/60">Modal / ConfirmModal / Drawer</div>
        <div class="flex flex-wrap gap-2">
          <Button size="sm" on:click={() => (modalOpen = true)}>打开 Modal</Button>
          <Button size="sm" variant="secondary" on:click={() => (confirmOpen = true)}>
            打开 Confirm
          </Button>
          <Button size="sm" variant="gray" on:click={() => (drawerOpen = true)}>
            打开 Drawer
          </Button>
        </div>
      </div>

      <div class="rounded-xl border border-base-300 bg-base-100 p-4 space-y-3">
        <div class="text-xs text-base-content/60">InfoTooltip</div>
        <div class="flex items-center gap-2 text-sm text-base-content">
          帮助信息
          <InfoTooltip content="这里展示提示信息，适合解释表单字段。" />
        </div>
      </div>
    </div>
  </section>

  <section class="space-y-4">
    <h2 class="text-base font-semibold text-base-content">数据展示</h2>
    <div class="grid gap-4 lg:grid-cols-2">
      <div class="rounded-xl border border-base-300 bg-base-100 p-4 space-y-3">
        <div class="text-xs text-base-content/60">Avatar / StatusLabel</div>
        <div class="flex items-center gap-4">
          <Avatar src="/logo-openai.png" size="md" />
          <Avatar letter="H" size="md" />
          <StatusLabel status="enabled" text="启用" />
          <StatusLabel status="idle" text="待机" />
          <StatusLabel status="disabled" text="禁用" />
          <StatusLabel status="error" text="异常" />
        </div>
      </div>

      <div class="rounded-xl border border-base-300 bg-base-100 p-4 space-y-3">
        <div class="text-xs text-base-content/60">ChatList</div>
        <div class="h-44 rounded-lg border border-base-300 overflow-hidden">
          <ChatList
            chats={chatSamples}
            activeId="chat-2"
            onChatClick={() => triggerToast("info")}
          />
        </div>
      </div>
    </div>
  </section>

  <section class="space-y-4">
    <h2 class="text-base font-semibold text-base-content">表格行组件</h2>
    <div class="rounded-xl border border-base-300 bg-base-100 p-4">
      <TableGroup title="基础组件" collapsible showDivider>
        <SwitchRow
          label="自动同步"
          bind:checked={tableToggle}
          description="打开后自动保存配置"
        />
        <SelectRow
          label="运行环境"
          options={selectOptions}
          bind:selectedValue={tableSelect}
          description="选择默认环境"
        />
        <NumberStepperRow
          label="重试次数"
          bind:value={tableNumber}
          min={0}
          max={5}
          step={1}
        />
        <LabeledSliderRow
          label="创造性"
          bind:value={labeledValue}
          min={0}
          max={1}
          step={0.1}
          leftLabel="保守"
          rightLabel="大胆"
          scaleMarks={[
            { value: 0, position: 0 },
            { value: 0.5, position: 50 },
            { value: 1, position: 100 }
          ]}
        />
        <TextareaRow
          label="说明"
          bind:value={tableTextarea}
          rows={3}
          showCharCount
          maxlength={80}
          description="支持多行输入"
        />
        <TextRow
          label="显示名称"
          bind:value={tableText}
          placeholder="输入名称"
        />
        <StatusLabelRow
          label="供应商状态"
          status="enabled"
          statusText="运行中"
          icon="AI"
          onclick={() => triggerToast("success")}
        />
        <DefaultRow
          label="进入高级设置"
          value="共 6 项"
          onclick={() => triggerToast("info")}
        />
      </TableGroup>
    </div>
  </section>
</div>

<Modal
  open={modalOpen}
  title="示例 Modal"
  onClose={() => (modalOpen = false)}
  closeOnBackdropClick
>
  <div class="max-w-lg bg-base-100 rounded-xl px-6 py-5">
    <div class="space-y-2">
      <h3 class="text-base font-medium text-base-content">Modal 内容</h3>
      <p class="text-sm text-base-content/70">
        这里可以放置表单、说明文字或操作按钮。
      </p>
      <div class="flex gap-2">
        <Button size="sm" variant="secondary" on:click={() => (modalOpen = false)}>
          关闭
        </Button>
        <Button size="sm" on:click={() => triggerToast("success")}>执行操作</Button>
      </div>
    </div>
  </div>
</Modal>

<ConfirmModal
  open={confirmOpen}
  title="确认删除"
  message="确认要删除这条记录吗？此操作不可撤销。"
  onConfirm={() => {
    triggerToast("success");
    confirmOpen = false;
  }}
  onCancel={() => (confirmOpen = false)}
  onClose={() => (confirmOpen = false)}
/>

<Drawer
  open={drawerOpen}
  title="侧边抽屉"
  onClose={() => (drawerOpen = false)}
>
  <div class="p-4 space-y-3">
    <p class="text-sm text-base-content/70">
      抽屉适合放置批量操作或辅助信息。
    </p>
    <Button size="sm" on:click={() => (drawerOpen = false)}>关闭</Button>
  </div>
</Drawer>
