<script lang="ts">
  import { onMount } from "svelte";
  import { app, init } from "./lib/state.svelte";
  import { language, t } from "./lib/i18n";
  import Nav from "./components/Nav.svelte";
  import Overview from "./pages/Overview.svelte";
  import Chatbox from "./pages/Chatbox.svelte";
  import Pulse from "./pages/Pulse.svelte";
  import Settings from "./pages/Settings.svelte";

  onMount(() => {
    init();
  });

  $effect(() => {
    document.documentElement.lang = language();
  });
</script>

{#if app.config && app.snap}
  <div class="shell">
    <Nav />
    <main>
      {#if app.error}
        <p class="banner" role="alert">{t("error.save")} {app.error}</p>
      {/if}
      {#if app.page === "overview"}
        <Overview />
      {:else if app.page === "chatbox"}
        <Chatbox />
      {:else if app.page === "pulse"}
        <Pulse />
      {:else}
        <Settings />
      {/if}
    </main>
  </div>
{:else}
  <div class="boot" role={app.error ? "alert" : "status"}>
    {#if app.error}
      <p>{t("error.start")}</p>
      <p class="muted small">{app.error}</p>
    {:else}
      <p class="muted">{t("loading")}</p>
    {/if}
  </div>
{/if}

<style>
  .shell {
    display: flex;
    height: 100%;
  }

  main {
    flex: 1;
    min-width: 0;
    padding: 28px 32px 32px;
    overflow-y: auto;
  }

  .banner {
    margin-bottom: 16px;
    padding: 10px 14px;
    border-radius: var(--r-control);
    border: 1px solid var(--warn);
    color: var(--warn);
    user-select: text;
  }

  .boot {
    height: 100%;
    display: grid;
    place-content: center;
    gap: 8px;
    text-align: center;
    user-select: text;
  }
</style>
