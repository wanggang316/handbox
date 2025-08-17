<script lang="ts">
  import "../../app.css";
  export let children;
</script>

<div class="settings-root">
  {@render children()}
</div>

<style>
  .settings-root {
    height: 100vh;
    width: 100vw;
    background: var(--bg-primary);
    color: var(--text-primary);
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
</style>
