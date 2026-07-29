<script lang="ts">
  import Modal from "../ui/Modal.svelte";
  import { TextRow, TableGroup } from "../ui/table";
  import Avatar from "../ui/Avatar.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import { t } from "$lib/i18n";

  interface Props {
    open?: boolean;
    user: {
      username: string;
      email: string;
      avatar: string;
    };
    onClose?: () => void;
    onSave?: (userData: {
      username: string;
      email: string;
      avatar?: string;
    }) => void;
  }

  let {
    open = false,
    user,
    onClose = () => {},
    onSave = () => {},
  }: Props = $props();

  let editedUsername = $state("");
  let editedEmail = $state("");
  let editedAvatar = $state("");

  let modalRef: Modal;

  $effect(() => {
    if (open && user) {
      console.log("EditProfileModal 打开，用户数据:", user);
      editedUsername = user.username || "";
      editedEmail = user.email || "";
      editedAvatar = user.avatar || "";
    }
  });

  $effect(() => {
    console.log("EditProfileModal open 状态:", open);
  });

  function handleSave() {
    onSave({
      username: editedUsername,
      email: editedEmail,
      avatar: editedAvatar,
    });
    modalRef?.handleClose();
  }

  function handleCancel() {
    editedUsername = user.username;
    editedEmail = user.email;
    editedAvatar = user.avatar;
    modalRef?.handleClose();
  }

  function handleAvatarChange(file: File) {
    console.log("头像文件选择:", file);
    // Avatar is kept locally as a data URL; no server upload.
    const reader = new FileReader();
    reader.onload = (e) => {
      editedAvatar = e.target?.result as string;
    };
    reader.readAsDataURL(file);
  }
</script>

<Modal bind:this={modalRef} {open} {onClose} showCloseButton={false}>
  <div class="relative flex flex-col p-8">
    <div class="flex justify-center py-4">
      <Avatar 
        src={editedAvatar || user.avatar} 
        letter={user.username?.charAt(0)}
        size="lg" 
        editable={true}
        onImageChange={handleAvatarChange}
      />
    </div>

    <div class="text-center mb-8">
      <p class="text-base text-base-content">
        {user.email}
      </p>
    </div>

    <div class="mb-8">
      <TableGroup>
        <TextRow
          label={t("settings.account.username")}
          bind:value={editedUsername}
          placeholder={t("settings.account.usernamePlaceholder")}
        />
      </TableGroup>
    </div>

    <div class="flex justify-end gap-4">
      <Button
        class="w-18"
        size="lg"
        variant="secondary"
        onclick={handleCancel}
      >{t("common.cancel")}</Button>

      <Button
        class="w-18"
        size="lg"
        onclick={handleSave}
      >{t("common.save")}</Button>
    </div>
  </div>
</Modal>
