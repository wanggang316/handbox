# UI Components

位置：`src/lib/components/ui` 与 `src/lib/components/ui/table`。

**使用约定**
- 组件样式以 Tailwind 为主，少量组件自带 `<style>`。
- 使用 Svelte 5 runes 的组件支持 `bind:` 语法（如 `Toggle`、`Textarea`、`Slider`）。
- 带有 `onclick` / `onChange` 等回调的组件，优先走对应 prop。按钮类组件已统一为 runes（`$props`），事件一律走 `onclick` prop，不再支持 `on:click` 事件转发。
- 颜色一律通过语义化 `variant` prop 表达，已移除 `bgColor` / `hoverColor` / `textColor` 这类 color-as-prop API。

## 按钮类
- `Button`：基础按钮。Props: `variant`（`primary` | `secondary` | `gray` | `danger` | `ghost` | `clear`）, `size`, `disabled`, `type`, `customClass`, `onclick`。
- `RoundButton`：圆角按钮。Props: `label`, `icon`, `loading`, `variant`（`primary` | `accent` | `danger` | `secondary`）, `size`, `rounded`, `fontSize`, `customClass`, `onclick`。
- `CircleButton`：圆形图标按钮。Props: `icon`, `iconSize`, `ariaLabel`, `variant`（`neutral`（默认）| `secondary`）, `size`, `rounded`, `customClass`, `onclick`。
- `IconButton`：方形图标按钮。Props: `icon`, `iconSize`, `strokeWidth`, `ariaLabel`, `variant`（`ghost`，默认）, `size`, `rounded`, `customClass`, `onclick`。
- `ArrowButton`：文本 + 下拉箭头。Props: `label`, `icon`, `iconSize`, `onclick`。
- `TrafficLightsRedButton`：窗口关闭按钮样式。Props: `onClick`。

示例：
```svelte
<Button variant="primary" onclick={handleClick}>保存</Button>
<RoundButton label="确认" variant="accent" onclick={handleConfirm} />
<CircleButton icon={Box} ariaLabel="图标按钮" />
```

## 表单类
- `Input`：文本输入框。Props: `label`, `placeholder`, `type`, `value`, `onInput`。
- `Textarea`：多行输入。Props: `value`, `placeholder`, `rows`, `disabled`, `readonly`, `maxlength`, `minlength`, `required`, `showCharCount`。
- `Select`：下拉框。Props: `options`, `value` / `selectedValue`, `placeholder`, `autoWidth`, `disabled`, `size`, `onChange` / `onSelect`。
- `Toggle`：开关。Props: `checked`, `onChange`, `onChangeBefore`, `id`, `disabled`。
- `Slider`：滑杆。Props: `value`, `min`, `max`, `step`, `label`, `formatValue`, `description`。
- `LabeledSlider`：带左右标签与刻度。Props: `value`, `min`, `max`, `step`, `leftLabel`, `rightLabel`, `scaleMarks`, `showValue`, `showScaleMarks`。
- `NumberStepper`：数字步进。Props: `value`, `min`, `max`, `step`, `defaultValue`, `placeholder`, `disabled`。

示例：
```svelte
<Toggle bind:checked={enabled} onChange={(v) => (enabled = v)} />
<Select options={options} bind:selectedValue={selected} onChange={handleSelect} />
<Textarea bind:value={content} rows={4} showCharCount />
```

## 导航与布局
- `Tabs`：标签切换。Props: `value`, `items`, `onChange`。
- `Menu`：菜单列表。Props: `items`, `activeId`, `onItemClick`, `containerClass`。
- `MenuButton`：菜单项按钮。Props: `title`, `isActive`, `icon`, `iconPosition`, `iconSize`, `onclick`, `buttonClass`, `activeClass`, `iconClass`, `icon_slot`（snippet）。
- `ResizableSidebar`：可拖拽侧栏。Props: `initialWidth`, `minWidth`, `maxWidth`, `storageKey`, `width`。

示例：
```svelte
<Tabs value={tab} items={items} onChange={(v) => (tab = v)} />
<Menu items={menuItems} activeId={activeId} onItemClick={handleMenu} />
```

## 反馈与弹层
- `Modal`：通用弹窗。Props: `open`, `title`, `showCloseButton`, `closeOnBackdropClick`, `onClose`。
- `ConfirmModal`：确认弹窗。Props: `open`, `title`, `message`, `confirmText`, `cancelText`, `onConfirm`, `onCancel`。
- `Drawer`：侧边抽屉。Props: `open`, `title`, `showCloseButton`, `onClose`。
- `Toast`：全局提示组件。通过 `toastActions` 调用，容器已在根布局挂载。
- `InfoTooltip`：帮助提示。Props: `content`, `size`。

示例：
```svelte
<Button onclick={() => (open = true)}>打开</Button>
<Modal open={open} title="标题" onClose={() => (open = false)} />
```

## 数据展示
- `Avatar`：头像。Props: `src`, `letter`, `size`, `editable`, `onImageChange`。
- `StatusLabel`：状态标签。Props: `status`, `text`。
- `ChatList`：聊天列表。Props: `chats`, `activeId`, `onChatClick`, `onRename`, `onDelete`, `onGenerateTitle`。

## 表格行组件
- `TableGroup`：分组容器。Props: `title`, `collapsible`, `defaultCollapsed`, `showDivider`。
- `TableBaseRow`：行基础结构。Props: `label`, `layout`, `py`, `rightContent`, `helpText`。
- `SwitchRow`：开关行。Props: `label`, `checked`, `description`, `helpText`, `disabled`, `onChange`。
- `SelectRow`：下拉行。Props: `label`, `options`, `selectedValue`, `description`, `helpText`, `disabled`, `onSelect`。
- `NumberStepperRow`：数字步进行。Props: `label`, `value`, `min`, `max`, `step`, `defaultValue`。
- `LabeledSliderRow`：标签滑杆行。Props: `label`, `value`, `min`, `max`, `step`, `leftLabel`, `rightLabel`, `scaleMarks`。
- `TextareaRow`：多行输入行。Props: `label`, `value`, `rows`, `maxlength`, `showCharCount`, `description`。
- `TextRow`：单行输入行。Props: `label`, `value`, `placeholder`, `readonly`, `isPassword`。
- `StatusLabelRow`：状态行。Props: `label`, `status`, `statusText`, `icon`, `iconSrc`, `clickable`, `onclick`。
- `DefaultRow`：默认跳转行。Props: `label`, `value`, `clickable`, `onclick`。

示例：
```svelte
<TableGroup title="配置">
  <SwitchRow label="自动同步" bind:checked={enabled} />
  <SelectRow label="环境" options={options} bind:selectedValue={env} />
</TableGroup>
```
