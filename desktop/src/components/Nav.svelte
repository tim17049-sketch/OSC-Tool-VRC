<script lang="ts">
  import { app, type Page } from "../lib/state.svelte";
  import { t, type Key } from "../lib/i18n";

  const items: { page: Page; label: Key; icon: string }[] = [
    {
      page: "overview",
      label: "nav.overview",
      icon: "M4 11.5 12 4l8 7.5V20a1 1 0 0 1-1 1h-4.5v-6h-5v6H5a1 1 0 0 1-1-1z",
    },
    {
      page: "chatbox",
      label: "nav.chatbox",
      icon: "M5 4h14a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-6l-4.5 4v-4H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z",
    },
    {
      page: "pulse",
      label: "nav.pulse",
      icon: "M12 20.5S3.5 15 3.5 9A4.5 4.5 0 0 1 12 7a4.5 4.5 0 0 1 8.5 2c0 6-8.5 11.5-8.5 11.5z",
    },
    {
      page: "settings",
      label: "nav.settings",
      icon: "M4 7h9m4 0h3M4 17h3m4 0h9M15 4.5v5M9 14.5v5",
    },
  ];
</script>

<nav aria-label="OSC Tool VRC">
  <div class="mark" aria-hidden="true">
    <svg viewBox="0 0 32 32" width="32" height="32">
      <rect x="2" y="3" width="28" height="21" rx="7" fill="var(--pulse)" />
      <path d="M11 23.5h8l-4 5.5z" fill="var(--pulse)" />
      <path
        d="M6.5 14h5l2-5 3.5 9.5 2.5-4.5h6"
        fill="none"
        stroke="#2a0b18"
        stroke-width="2.2"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>
  </div>
  {#each items as item}
    <button
      type="button"
      class:active={app.page === item.page}
      aria-current={app.page === item.page ? "page" : undefined}
      onclick={() => (app.page = item.page)}
    >
      <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
        <path
          d={item.icon}
          fill="none"
          stroke="currentColor"
          stroke-width="1.7"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
      <span>{t(item.label)}</span>
    </button>
  {/each}
</nav>

<style>
  nav {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 4px;
    width: 104px;
    padding: 18px 10px;
    border-right: 1px solid var(--line);
    background: var(--panel);
    flex: none;
  }

  .mark {
    display: grid;
    place-items: center;
    height: 44px;
    margin-bottom: 14px;
  }

  button {
    display: grid;
    justify-items: center;
    gap: 5px;
    padding: 11px 4px 9px;
    border: 0;
    border-radius: 12px;
    background: none;
    color: var(--muted);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: color 120ms ease-out, background 120ms ease-out;
  }

  button:hover {
    color: var(--text);
  }

  button.active {
    color: var(--text);
    background: var(--raised);
  }

  button.active svg {
    color: var(--pulse);
  }
</style>
