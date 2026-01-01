<script lang="ts">
  import Avatar from "../ui/Avatar.svelte";

  // 用户信息接口
  interface UserInfo {
    isLoggedIn: boolean;
    username?: string;
    avatar?: string;
    isPro?: boolean;
  }

  interface Props {
    user?: UserInfo;
    onUserClick?: () => void;
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
  onclick={onUserClick}
  onkeydown={(e) => e.key === "Enter" && onUserClick()}
  role="button"
  tabindex="0"
>
  <!-- 头像 -->
  <div class="flex flex-row gap-2 items-center">
    <Avatar src={user.avatar} letter={user.username} size="sm" />
  </div>

  <!-- 用户信息 -->
  <div class="leading-[1.4] flex-1">
    <div class="text-[14px] text-base-content/80">{displayName}</div>
    <div
      class={`text-[12px] ${user.isLoggedIn ? "text-base-content/60" : "text-primary"}`}
    >
      {planText}
    </div>
  </div>
</div>
