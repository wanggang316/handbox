<script lang="ts">
  import Modal from "../ui/Modal.svelte";
  import { TextRow, TableGroup } from "../ui/table";
  import Avatar from "../ui/Avatar.svelte";
  import RoundButton from "../ui/RoundButton.svelte";

  export let open = false;
  export let user: {
    username: string;
    email: string;
    avatar: string;
  };
  export let onClose: () => void = () => {};
  export let onSave: (userData: {
    username: string;
    email: string;
    avatar?: string;
  }) => void = () => {};

  // 本地编辑状态
  let editedUsername = "";
  let editedEmail = "";
  let editedAvatar = "";
  
  // Modal 引用
  let modalRef: Modal;

  // 重置表单数据
  $: if (open && user) {
    console.log("EditProfileModal 打开，用户数据:", user);
    editedUsername = user.username || "";
    editedEmail = user.email || "";
    editedAvatar = user.avatar || "";
  }

  // 调试 open 状态变化
  $: console.log("EditProfileModal open 状态:", open);

  function handleSave() {
    onSave({
      username: editedUsername,
      email: editedEmail,
      avatar: editedAvatar,
    });
    modalRef?.handleClose();
  }

  function handleCancel() {
    // 重置表单数据
    editedUsername = user.username;
    editedEmail = user.email;
    editedAvatar = user.avatar;
    modalRef?.handleClose();
  }

  function handleAvatarChange(file: File) {
    console.log("头像文件选择:", file);
    // 这里可以添加上传头像的逻辑
    // 例如：转换为 base64 或上传到服务器
    const reader = new FileReader();
    reader.onload = (e) => {
      editedAvatar = e.target?.result as string;
    };
    reader.readAsDataURL(file);
  }
</script>

<Modal bind:this={modalRef} {open} {onClose} showCloseButton={false}>
  <div class="relative flex flex-col p-8">
    <!-- 用户头像 -->
    <div class="flex justify-center py-4">
      <Avatar 
        src={editedAvatar || user.avatar} 
        letter={user.username?.charAt(0)}
        size="lg" 
        editable={true}
        onImageChange={handleAvatarChange}
      />
    </div>

    <!-- 邮箱显示 -->
    <div class="text-center mb-8">
      <p class="text-base text-base-content">
        {user.email}
      </p>
    </div>

    <!-- 用户名输入框 -->
    <div class="mb-8">
      <TableGroup>
        <TextRow
          label="用户名"
          bind:value={editedUsername}
          placeholder="请输入"
        />
      </TableGroup>
    </div>

    <div class="flex justify-end gap-4">
      <RoundButton
        customClass="w-18"
        label="取消"
        variant="secondary"
        onclick={handleCancel}
      />

      <RoundButton 
        customClass="w-18" 
        label="保存" 
        onclick={handleSave} 
      />
    </div>
  </div>
</Modal>
