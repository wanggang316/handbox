<script lang="ts">
  import Avatar from "../ui/Avatar.svelte";
  import { Settings } from "@lucide/svelte";

  // 用户信息接口
  interface UserInfo {
    isLoggedIn: boolean;
    username?: string;
    avatar?: string;
    isPro?: boolean;
  }

  interface Props {
    user?: UserInfo;
    onUserClick?: (event: MouseEvent | KeyboardEvent) => void;
  }

  let { user = { isLoggedIn: false }, onUserClick = () => {} }: Props =
    $props();

  const displayName = $derived(
    user.isLoggedIn ? user.username || "用户" : "未登录"
  );
  const planText = $derived(
    user.isLoggedIn ? (user.isPro ? "Pro Plan" : "Free Plan") : ""
  );
</script>

<div
  class="flex items-center justify-center gap-3 py-1 px-2 cursor-pointer hover:bg-base-300 transition-colors rounded-lg"
  onclick={(event) => onUserClick(event)}
  onkeydown={(e) => e.key === "Enter" && onUserClick(e)}
  role="button"
  tabindex="0"
>
  <!-- 头像 -->
  {#if user.isLoggedIn}
    <div class="flex flex-row gap-2 items-center">
      <Avatar
        src={user.avatar}
        letter={user.username}
        size="sm"
        class="pointer-events-none"
      />
    </div>

    <!-- 用户信息 -->
    <div class="leading-[1.4] flex-1">
      <div class="text-[14px] text-base-content/80">{displayName}</div>
      <div class={`text-[12px] text-base-content/60`}>{planText}</div>
    </div>
  {:else}
    <div class="flex items-center gap-2 flex-1 justify-start">
      <Settings size={16} class="text-base-content/70" />
      <div class="text-[14px] text-base-content/80">设置</div>
    </div>
  {/if}
</div>
