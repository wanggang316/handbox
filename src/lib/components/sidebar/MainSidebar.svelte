<script lang="ts">
  import { chatState, chatActions } from "$lib/states/chat.svelte";
  import { browser } from "$app/environment";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import ChatList from "$lib/components/ui/ChatList.svelte";
  import MenuButton from "$lib/components/ui/MenuButton.svelte";
  import RoundButton from "$lib/components/ui/RoundButton.svelte";
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import UserSidebar from "$lib/components/sidebar/UserSidebar.svelte";
  import { Box, Plus, Search } from "@lucide/svelte";
  import { openSettingsWindow } from "$lib/api/window";
  import { authState, login } from "$lib/states/auth.svelte";

  // 获取当前选中的聊天 ID
  let currentChatId = $derived(
    browser && $page.url ? $page.url.searchParams.get("id") || "" : ""
  );

  // 将真实聊天数据转换为 Menu 组件期望的格式
  let chats = $derived(
    chatState.chats
      .filter((chat) => chat.id) // 过滤掉没有 id 的聊天
      .map((chat) => ({
        id: chat.id!,
        title: chat.name,
      }))
  );

  function handleChatClick(chat: any) {
    console.log("Clicked chat:", chat);
    // 使用 SvelteKit 的客户端路由导航，避免页面重新加载
    goto(`/chat?id=${chat.id}`);
  }

  function handleArtifactClick() {
    console.log("Clicked artifact menu");
    goto(`/artifacts`);
  }

  function handleNewChatClick() {
    console.log("Clicked new chat");
    goto("/chat");
  }

  function handleSearchClick() {
    console.log("Clicked search");
    // 导航到搜索页面
    goto("/search");
  }

  async function handleUserClick() {
    if (currentUser.isLoggedIn) {
      console.log("打开用户设置");
      // 打开独立的设置窗口
      openSettingsWindow().catch((err) => {
        console.error("Failed to open settings window:", err);
      });
    } else {
      // 直接启动 Google OAuth 登录流程
      await login();
    }
  }

  // 处理聊天重命名
  async function handleChatRename(chat: any, newName: string) {
    try {
      await chatActions.renameChat(chat.id, newName);
      console.log("Chat renamed successfully:", chat.id, newName);
    } catch (error) {
      console.error("Failed to rename chat:", error);
      // 这里可以显示错误提示
    }
  }

  // 处理聊天删除
  async function handleChatDelete(chat: any) {
    try {
      await chatActions.deleteChat(chat.id);
      console.log("Chat deleted successfully:", chat.id);

      // 如果删除的是当前聊天，跳转到默认页面
      if (currentChatId === chat.id) {
        goto("/chat");
      }
    } catch (error) {
      console.error("Failed to delete chat:", error);
      // 这里可以显示错误提示
    }
  }

  // 处理生成标题（接收ChatList组件生成的标题）
  async function handleGenerateTitle(chat: any, newTitle: string) {
    try {
      await chatActions.renameChat(chat.id, newTitle);
      console.log("Chat title updated successfully:", chat.id, newTitle);
    } catch (error) {
      console.error("Failed to update generated title:", error);
      // 这里可以显示错误提示
    }
  }

  // 从 authState 获取用户状态
  const currentUser = $derived({
    isLoggedIn: authState.isLoggedIn,
    username: authState.user?.username,
    email: authState.user?.email,
    avatar: authState.user?.avatar,
    isPro: authState.user?.isPro || false
  });
</script>

<div
  class="h-full flex flex-col bg-base-200 p-0 pt-15 rounded-2xl overflow-hidden"
>
  <!-- 顶部固定区域 -->
  <div class="flex-shrink-0 space-y-6 mb-6">
    <!-- 顶部操作 -->
    <div class="flex gap-2 px-4">
      <RoundButton
        customClass="flex-1"
        label="New chat"
        icon={Plus}
        onclick={handleNewChatClick}
      />
      <CircleButton
        icon={Search}
        ariaLabel="搜索"
        onclick={handleSearchClick}
      />
    </div>

    <div class="flex px-2">
      <MenuButton
        title="Artifacts"
        icon={Box}
        iconSize={20}
        onClick={() => handleArtifactClick()}
      />
    </div>
  </div>

  <!-- 中间可滚动区域 -->
  <div class="flex-1 min-h-0">
    <ChatList
      {chats}
      activeId={currentChatId}
      onChatClick={handleChatClick}
      onRename={handleChatRename}
      onDelete={handleChatDelete}
      onGenerateTitle={handleGenerateTitle}
    />
  </div>

  <!-- 用户信息 -->
  <div class="flex-shrink-0 p-2">
    <UserSidebar user={currentUser} onUserClick={handleUserClick} />
  </div>
</div>
