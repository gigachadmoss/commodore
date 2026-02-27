<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import { MissingDep } from "./MissingDep";

  import "../app.css";
  import "@sakun/system.css";
  
  import WindowContent from "./WindowContent.svelte";
  import DepInstallRequired from "./DepInstallRequired.svelte";

  const appWindow = getCurrentWindow();

  let os: string = "";
  let depInstallRequired = MissingDep.None;

  (async () => {
    os = (await invoke("get_os")) as string;
  })();
  
  (async () => {
    if (os === "windows" || os === "linux") {
      const checkSevenZip = async () => {
        if (!await invoke("sevenzip_installed")) {
          depInstallRequired = MissingDep.SevenZip;
          setTimeout(checkSevenZip, 2000);
        } else {
          depInstallRequired = MissingDep.None;
        }
      };
      checkSevenZip();
    }
  })();
</script>

<style>
  .window {
    margin: 0;
    height: 100%;
    border-width: 2px;
  }
</style>

<main class="select-none dark:invert h-screen">
  <div class="window min-h-full">
    <div data-tauri-drag-region class="title-bar">
      <button aria-label="Close" onclick={() => {appWindow.close()}} class="close"></button>
      <h1 data-tauri-drag-region class="title content-center">Commodore</h1>
      <div class="w-10"></div> <!-- Fills in the space where the resize button usually is -->
    </div>
    <div class="separator"></div>
    <div class="window-pane w-full h-full">
      {#if depInstallRequired === MissingDep.None}
        <WindowContent os={os} />
      {:else}
        <DepInstallRequired bind:depInstallRequired os={os} />
      {/if}
    </div>
  </div>
</main>
