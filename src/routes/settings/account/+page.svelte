<script lang="ts">
  import UserAccount from "$lib/components/settings/UserAccount.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import AccountEdit from "$lib/components/settings/AccountEdit.svelte";
  import GoogleLoginButton from "$lib/components/auth/GoogleLoginButton.svelte";
  import { authState, logout as authLogout } from "$lib/states/auth.svelte";
  import { updateUserProfile } from "$lib/api/auth";
  import { AppError } from "$lib/api";

  // Modal 状态控制
  let showEditModal = $state(false);
  let isLoading = $state(false);
  let errorMessage = $state<string | null>(null);

  // 从 authState 获取用户状态
  const user = $derived({
    isLoggedIn: authState.isLoggedIn,
    username: authState.user?.username,
    email: authState.user?.email,
    avatar: authState.user?.avatar,
    isPro: authState.user?.isPro || false,
  });

  function handleEditProfile() {
    showEditModal = true;
  }

  function handleCloseModal() {
    showEditModal = false;
    errorMessage = null;
  }

  async function handleSaveProfile(userData: {
    username: string;
    email: string;
    avatar?: string;
  }) {
    if (!authState.isLoggedIn) return;

    isLoading = true;
    errorMessage = null;

    try {
      // 调用后端更新用户资料
      const updatedUser = await updateUserProfile({
        username: userData.username,
        avatar: userData.avatar,
      });

      // 更新认证状态
      authState.user = updatedUser;

      // 关闭弹窗
      showEditModal = false;
    } catch (error) {
      console.error("更新用户资料失败:", error);

      if (error instanceof AppError) {
        errorMessage = error.message;
      } else {
        errorMessage = "更新失败，请重试";
      }
    } finally {
      isLoading = false;
    }
  }

  async function handleLogout() {
    if (!confirm("确定要退出登录吗？")) {
      return;
    }

    isLoading = true;
    errorMessage = null;

    try {
      await authLogout();
      console.log("退出登录成功");
    } catch (error) {
      console.error("退出登录失败:", error);
      errorMessage = "退出失败，请重试";
    } finally {
      isLoading = false;
    }
  }
</script>

<div class="mt-8 p-6 pr-8 flex flex-col gap-y-4">
  <!-- 错误提示 -->
  {#if errorMessage}
    <div class="p-4 bg-error/10 border border-error/20 rounded-lg">
      <p class="text-sm text-error">{errorMessage}</p>
    </div>
  {/if}

  <!-- 用户信息卡片 -->
  {#if user.isLoggedIn}
    <TableGroup>
      <div class="px-6 py-6 flex flex-row gap-y-4">
        <div class="flex-1">
          <UserAccount {user} />
        </div>
        {#if user.isLoggedIn}
          <div class="flex items-center">
            <Button
              variant="gray"
              size="sm"
              on:click={handleEditProfile}
              disabled={isLoading}
            >
              编辑资料
            </Button>
          </div>
        {/if}
      </div>
    </TableGroup>

    <!-- 退出登录按钮 -->
    <div>
      <Button
        variant="gray"
        size="sm"
        on:click={handleLogout}
        disabled={isLoading}
      >
        {isLoading ? "退出中..." : "退出登录"}
      </Button>
    </div>
  {:else}
    <!-- Google 登录按钮 -->
    <div class="max-w-md">
      <GoogleLoginButton />
    </div>
  {/if}
</div>

<!-- 编辑资料弹窗 -->
{#if user.isLoggedIn && authState.user}
  <AccountEdit
    open={showEditModal}
    user={{
      username: authState.user.username,
      email: authState.user.email,
      avatar: authState.user.avatar || "",
    }}
    onClose={handleCloseModal}
    onSave={handleSaveProfile}
  />
{/if}
