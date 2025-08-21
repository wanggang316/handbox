<script lang="ts">
  import UserAccount from "$lib/components/ui/UserAccount.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import AccountEdit from "$lib/components/settings/AccountEdit.svelte";

  // 示例用户数据
  let user = {
    isLoggedIn: true,
    username: "wanggang",
    email: "gumpwang2016@gmail.com",
    avatar: "https://lh3.googleusercontent.com/a/ACg8ocKdKLfYXuyg3WFnA4HGTrga_E2YtSw_r9x3079cyaNFsHSwsYAh=s96-c",
    isPro: true,
  };

  // Modal 状态控制
  let showEditModal = false;

  function handleEditProfile() {
    console.log("编辑用户资料 - 点击事件触发");
    console.log("当前 showEditModal 状态:", showEditModal);
    showEditModal = true;
    console.log("设置后 showEditModal 状态:", showEditModal);
  }

  function handleCloseModal() {
    showEditModal = false;
  }

  function handleSaveProfile(userData: { username: string; email: string; avatar?: string }) {
    console.log("保存用户资料", userData);
    // 更新用户数据
    user.username = userData.username;
    user.email = userData.email;
    if (userData.avatar) {
      user.avatar = userData.avatar;
    }
    // 这里可以添加保存到后端的逻辑
  }

  function handleLogin() {
    console.log("用户登录");
    // 这里可以添加用户登录的逻辑
  }

  function handleLogout() {
    console.log("用户退出登录");
    // 这里可以添加用户退出登录的逻辑
    user.isLoggedIn = false;
    user.username = "";
    user.avatar = "";
  }
</script>

<div class="p-6 pr-8 flex flex-col gap-y-4">
  <!-- 已登录状态 -->
  <TableGroup>
    <div class="px-6 py-6 flex flex-row gap-y-4">
      <div class="flex-1">
        <UserAccount {user} />
      </div>
      {#if user.isLoggedIn}
        <div class="flex items-center">
          <Button variant="gray" size="sm" on:click={handleEditProfile}>
            编辑资料
          </Button>
        </div>
      {/if}
    </div>
  </TableGroup>

  {#if user.isLoggedIn}
    <!-- 退出登录按钮 -->
    <div>
      <Button variant="gray" size="sm" on:click={handleLogout}>退出登录</Button>
    </div>
  {:else}
    <div>
      <Button variant="gray" size="sm" on:click={handleLogin}>登录</Button>
    </div>
  {/if}
</div>

<!-- 编辑资料弹窗 -->
<AccountEdit 
  open={showEditModal} 
  {user}
  onClose={handleCloseModal}
  onSave={handleSaveProfile}
/>
