<script lang="ts">
  import { app, updateConfig } from "../lib/state.svelte";
  import { backend } from "../lib/backend";
  import { num, t } from "../lib/i18n";
  import type { MonitorRow, ParamFavorite, ParamKind } from "../lib/types";
  import Switch from "../components/Switch.svelte";

  const PREFIX = "/avatar/parameters/";
  const KINDS: { kind: ParamKind; label: string }[] = [
    { kind: "bool", label: "Bool" },
    { kind: "int", label: "Int" },
    { kind: "float", label: "Float" },
  ];

  const config = $derived(app.config!);
  const receiver = $derived(app.snap!.receiver);

  let name = $state("");
  let kind = $state<ParamKind>("bool");
  let value = $state(1);
  let status = $state<{ text: string; bad: boolean } | null>(null);

  const cleanName = $derived(name.trim().replace(/^\/avatar\/parameters\//, ""));
  const validName = $derived(cleanName !== "" && !/[\s#*,?[\]{}]/.test(cleanName));

  function show(kind: ParamKind, v: number): string {
    if (kind === "bool") return v ? "true" : "false";
    if (kind === "int") return String(Math.round(v));
    return num(Math.round(v * 100) / 100);
  }

  function setKind(next: ParamKind) {
    kind = next;
    value = next === "bool" ? 1 : 0;
  }

  async function send(fav?: ParamFavorite) {
    const p = fav ?? { name: cleanName, kind, value };
    if (!fav && !validName) {
      status = { text: t("osc.badName"), bad: true };
      return;
    }
    try {
      await backend.sendParam(p.name, p.kind, p.value);
      status = { text: t("osc.sent", { name: p.name, value: show(p.kind, p.value) }), bad: false };
    } catch (e) {
      status = { text: String(e), bad: true };
    }
  }

  function save() {
    if (!validName) {
      status = { text: t("osc.badName"), bad: true };
      return;
    }
    const fav = { name: cleanName, kind, value };
    const rest = config.paramFavorites.filter((f) => !(f.name === fav.name && f.kind === fav.kind && f.value === fav.value));
    updateConfig({ paramFavorites: [...rest, fav] });
  }

  function load(fav: ParamFavorite) {
    name = fav.name;
    kind = fav.kind;
    value = fav.value;
  }

  function remove(index: number) {
    updateConfig({ paramFavorites: config.paramFavorites.filter((_, i) => i !== index) });
  }

  // ---- monitor ----
  let rows = $state<MonitorRow[]>([]);
  let filter = $state("");
  const shown = $derived(
    rows.filter((r) => r.address.toLowerCase().includes(filter.trim().toLowerCase())),
  );

  $effect(() => {
    if (!receiver.enabled) return;
    let alive = true;
    const poll = async () => {
      try {
        const next = await backend.getMonitor();
        if (alive) rows = next;
      } catch {
        // The next poll tries again.
      }
    };
    poll();
    const timer = setInterval(poll, 500);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  });

  function pick(row: MonitorRow) {
    if (!row.address.startsWith(PREFIX)) return;
    name = row.address.slice(PREFIX.length);
    if (row.value === "true" || row.value === "false") {
      kind = "bool";
      value = row.value === "true" ? 1 : 0;
    } else if (/^-?\d+$/.test(row.value)) {
      kind = "int";
      value = Number(row.value);
    } else {
      kind = "float";
      value = Number(row.value) || 0;
    }
    status = null;
  }

  function age(ms: number): string {
    if (ms < 1000) return "< 1 s";
    if (ms < 60_000) return `${Math.floor(ms / 1000)} s`;
    return `${Math.floor(ms / 60_000)} min`;
  }

  function setPort(input: HTMLInputElement) {
    const port = Math.round(input.valueAsNumber);
    if (port >= 1 && port <= 65535) updateConfig({ oscReceivePort: port });
    else input.value = String(config.oscReceivePort);
  }
</script>

<div class="page">
  <h1>{t("osc.title")}</h1>

  <div class="columns">
    <section class="panel sender" aria-labelledby="send-title">
      <h2 id="send-title">{t("osc.send")}</h2>
      <p class="small muted">{t("osc.sendHelp")}</p>

      <form
        onsubmit={(e) => {
          e.preventDefault();
          send();
        }}
      >
        <label class="field">
          <span>{t("osc.name")}</span>
          <input
            class="input"
            type="text"
            spellcheck="false"
            autocomplete="off"
            placeholder={t("osc.namePlaceholder")}
            bind:value={name}
            oninput={() => (status = null)}
          />
        </label>

        <div class="field">
          <span id="kind-label">{t("osc.kind")}</span>
          <div class="segmented" role="radiogroup" aria-labelledby="kind-label">
            {#each KINDS as k}
              <button
                type="button"
                role="radio"
                aria-checked={kind === k.kind}
                onclick={() => setKind(k.kind)}
              >
                {k.label}
              </button>
            {/each}
          </div>
        </div>

        <div class="field">
          <span id="value-label">{t("osc.value")}</span>
          {#if kind === "bool"}
            <Switch checked={value !== 0} label={t("osc.on")} onchange={(on) => (value = on ? 1 : 0)} />
          {:else if kind === "int"}
            <input
              class="input num int"
              type="number"
              min="0"
              max="255"
              step="1"
              aria-labelledby="value-label"
              value={value}
              onchange={(e) => (value = Math.max(0, Math.min(255, Math.round(e.currentTarget.valueAsNumber || 0))))}
            />
          {:else}
            <div class="float">
              <input
                type="range"
                min="-1"
                max="1"
                step="0.01"
                aria-labelledby="value-label"
                bind:value
              />
              <b class="num">{show("float", value)}</b>
            </div>
          {/if}
        </div>

        <div class="actions">
          <button type="submit" class="btn primary">{t("osc.sendNow")}</button>
          <button type="button" class="btn" onclick={save}>{t("osc.save")}</button>
        </div>
        {#if status}
          <p class="small" class:warn={status.bad} class:ok={!status.bad} role="status">{status.text}</p>
        {/if}
      </form>
    </section>

    <section class="panel" aria-labelledby="fav-title">
      <h2 id="fav-title">{t("osc.favorites")}</h2>
      {#if config.paramFavorites.length}
        <ul class="favs">
          {#each config.paramFavorites as fav, i}
            <li>
              <button
                type="button"
                class="load"
                title={t("osc.favLoad", { name: fav.name })}
                onclick={() => load(fav)}
              >
                <span class="fav-name">{fav.name}</span>
                <span class="small muted num">{fav.kind} · {show(fav.kind, fav.value)}</span>
              </button>
              <button type="button" class="btn send" aria-label={t("osc.favSend", { name: fav.name })} onclick={() => send(fav)}>
                {t("osc.sendNow")}
              </button>
              <button
                type="button"
                class="icon"
                aria-label={t("osc.favDelete", { name: fav.name })}
                title={t("osc.favDelete", { name: fav.name })}
                onclick={() => remove(i)}
              >
                <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                  <path d="M6 6l12 12M18 6 6 18" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
                </svg>
              </button>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="small muted empty">{t("osc.noFavorites")}</p>
      {/if}
    </section>
  </div>

  <section class="panel monitor" aria-labelledby="recv-title">
    <header>
      <h2 id="recv-title">{t("osc.receive")}</h2>
      <Switch
        checked={config.oscReceive}
        label={t("osc.receiveToggle")}
        onchange={(oscReceive) => updateConfig({ oscReceive })}
      />
    </header>
    <p class="small muted">{t("osc.receiveHelp")}</p>

    <div class="bar">
      <label class="field port">
        <span>{t("osc.port")}</span>
        <input
          class="input num"
          type="number"
          min="1"
          max="65535"
          value={config.oscReceivePort}
          onchange={(e) => setPort(e.currentTarget)}
        />
      </label>
      <label class="field grow">
        <span>{t("osc.filter")}</span>
        <input
          class="input"
          type="search"
          spellcheck="false"
          placeholder={t("osc.filterPlaceholder")}
          bind:value={filter}
          disabled={!receiver.enabled}
        />
      </label>
      <button
        type="button"
        class="btn"
        disabled={!rows.length}
        onclick={async () => {
          await backend.clearMonitor();
          rows = [];
        }}
      >
        {t("osc.clear")}
      </button>
    </div>

    <p class="small state" class:warn={receiver.error}>
      {#if !receiver.enabled}
        <span class="dot"></span><span class="muted">{t("osc.off")}</span>
      {:else if receiver.error}
        <span class="dot warn"></span>{t("osc.portError", { port: receiver.port, error: receiver.error })}
      {:else}
        <span class="dot live"></span>
        <span class="muted">{t("osc.listening", { port: receiver.port, n: receiver.total })}</span>
      {/if}
    </p>

    {#if receiver.enabled}
      {#if shown.length}
        <p class="small muted">{t("osc.pick")}</p>
        <div class="scroll">
          <table>
            <thead>
              <tr>
                <th>{t("osc.address")}</th>
                <th>{t("osc.value")}</th>
                <th>{t("osc.count")}</th>
                <th>{t("osc.age")}</th>
              </tr>
            </thead>
            <tbody>
              {#each shown as row (row.address)}
                {@const param = row.address.startsWith(PREFIX)}
                <tr class:fresh={row.ageMs < 1000} class:param>
                  <td class="addr">
                    {#if param}
                      <button type="button" class="pick" onclick={() => pick(row)}>
                        <span class="muted">{PREFIX}</span>{row.address.slice(PREFIX.length)}
                      </button>
                    {:else}
                      {row.address}
                    {/if}
                  </td>
                  <td class="num">{row.value}</td>
                  <td class="num">{row.count}</td>
                  <td class="num muted">{age(row.ageMs)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {:else}
        <p class="small muted">{rows.length ? t("osc.noMatch") : t("osc.waiting")}</p>
      {/if}
    {/if}
  </section>
</div>

<style>
  .columns {
    display: grid;
    grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr);
    gap: 20px;
    align-items: start;
  }

  section {
    display: grid;
    gap: 12px;
    align-content: start;
  }

  form {
    display: grid;
    gap: 14px;
  }

  .segmented {
    display: inline-flex;
    width: fit-content;
    padding: 3px;
    gap: 3px;
    border-radius: var(--r-control);
    background: var(--ink);
    border: 1px solid var(--line);
  }

  .segmented button {
    min-width: 72px;
    height: 32px;
    border: 0;
    border-radius: 7px;
    background: none;
    color: var(--muted);
    font-weight: 600;
    cursor: pointer;
  }

  .segmented button[aria-checked="true"] {
    background: var(--raised);
    color: var(--text);
  }

  .int {
    width: 120px;
  }

  .float {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .float b {
    min-width: 44px;
    text-align: right;
  }

  .actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }

  .ok {
    color: var(--signal);
  }

  .warn {
    color: var(--warn);
  }

  .favs {
    display: grid;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .favs li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 0;
    border-top: 1px solid var(--line);
  }

  .load {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 0;
    padding: 4px 6px;
    border: 0;
    border-radius: 8px;
    background: none;
    text-align: left;
    cursor: pointer;
  }

  .load:hover {
    background: var(--raised);
  }

  .fav-name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .send {
    height: 34px;
    padding: 0 14px;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }

  .icon:hover {
    color: var(--text);
    background: var(--raised);
  }

  .empty {
    margin-top: 4px;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .bar {
    display: flex;
    align-items: end;
    gap: 12px;
  }

  .port {
    width: 120px;
  }

  .grow {
    flex: 1;
  }

  .state {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .scroll {
    max-height: 360px;
    overflow-y: auto;
    border-top: 1px solid var(--line);
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 14px;
    user-select: text;
  }

  th {
    position: sticky;
    top: 0;
    background: var(--panel);
    text-align: left;
    font-size: 13px;
    font-weight: 600;
    color: var(--muted);
    padding: 8px 8px 8px 0;
  }

  td {
    padding: 6px 8px 6px 0;
    border-top: 1px solid var(--line);
    transition: color 400ms ease-out;
  }

  th:not(:first-child),
  td:not(:first-child) {
    text-align: right;
    white-space: nowrap;
  }

  .addr {
    word-break: break-all;
  }

  tr.param:hover td {
    background: var(--raised);
  }

  tr.fresh td:nth-child(2) {
    color: var(--signal);
  }

  .pick {
    width: 100%;
    padding: 0;
    border: 0;
    background: none;
    text-align: left;
    cursor: pointer;
  }

  @media (max-width: 880px) {
    .columns {
      grid-template-columns: 1fr;
    }
  }
</style>
