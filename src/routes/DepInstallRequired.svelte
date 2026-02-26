<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { message } from '@tauri-apps/plugin-dialog';
  import { openUrl } from "@tauri-apps/plugin-opener";

  import { MissingDep } from "./MissingDep";

  export let depInstallRequired: MissingDep = MissingDep.None;
  export let os: string;

  let sevenZipInstallStatus: string = "";

  function open7zip() {
    openUrl("https://www.7-zip.org/");
  }

  async function install7zip() {
    sevenZipInstallStatus = "Downloading..."

    await invoke("install_sevenzip")
      .catch(async (e) => {
        await message(e as string, { title: "7-Zip Install Error", kind: "error"});
      });

    const install_unsub = await listen('7zip_install_started', () => {})

    sevenZipInstallStatus = "Installing..."

    install_unsub()
  }
</script>

<div class="flex flex-col h-full gap-2">
  {#if depInstallRequired === MissingDep.None}
    <p>All Dependencies Satisfied</p>
  {:else if depInstallRequired === MissingDep.SevenZip}
    <p>7-Zip must be installed</p>
  {/if}
  <div class="flex flex-row justify-end h-8 gap-2">
    {#if depInstallRequired === MissingDep.SevenZip}
      <button onclick={open7zip} class="btn inline-flex items-center justify-center h-full" >7-zip.org</button>
      {#if os === "windows"}
        <button onclick={install7zip} class="btn inline-flex items-center justify-center h-full">Install</button>
      {/if}
    {/if}
  </div>
</div>
