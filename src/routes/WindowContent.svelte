<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { message } from '@tauri-apps/plugin-dialog';

  export let os: string;

  let model: string = "";

  let status: string = "Ready";
  let busy: boolean = false;

  async function pull_drivers(install: boolean) {
    busy = true;
    
    if (install) {
      status = "Downloading & Installing...";
    } else {
      status = "Downloading...";
    }

    await invoke("pull_drivers", { install: install, model: model || undefined })
      .catch(async (e) => {
        await message(e as string + "\n\nCheck the logs for more details.", { title: "brigadier Error", kind: "error"});
      });

    busy = false;
    status = "Ready";
  }

  function kill_brigadier() {
    invoke("kill_brigadier");
  }
</script>

<div class="flex flex-col h-full gap-2">
  <p>{status}</p>
  <div class="flex flex-row h-8 gap-2">
    <input placeholder="Model (Auto)" bind:value={model} class="w-48" />
    <div class="w-full"></div>
    {#if !busy}
      <button onclick={() => pull_drivers(false)} class="btn inline-flex items-center justify-center h-full" >Save</button>
      {#if os === "windows"}
        {#if model === ""}
          <button onclick={() => pull_drivers(true)} class="btn inline-flex items-center justify-center h-full">Install</button>
        {/if}
      {/if}
    {:else}
      <button onclick={kill_brigadier} class="btn inline-flex items-center justify-center h-full" >Cancel</button>
    {/if}
  </div>
</div>
