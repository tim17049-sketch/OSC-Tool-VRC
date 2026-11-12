<script lang="ts">
  import { app, updateConfig } from "../lib/state.svelte";
  import { backend } from "../lib/backend";
  import { t } from "../lib/i18n";
  import { DEFAULT_SCAN_PARAM } from "../lib/types";
  import Switch from "../components/Switch.svelte";

  const config = $derived(app.config!);
  const scanner = $derived(app.snap!.scanner);
  const receiver = $derived(app.snap!.receiver);
  const latest = $derived(scanner.results[0] ?? null);

  function setParam(input: HTMLInputElement) {
    const value = input.value.trim().replace(/^\/avatar\/parameters\//, "");
    if (value && !/[\s#*,?[\]{}]/.test(value)) updateConfig({ scanParam: value });
    else input.value = config.scanParam;
  }
</script>

<div class="page">
  <h1>{t("scan.title")}</h1>

  <div class="columns">
    <section class="panel scan" aria-labelledby="scan-title">
      <h2 id="scan-title" class="sr">{t("scan.title")}</h2>
      <p class="small muted">{t("scan.help")}</p>

      <form
        onsubmit={(e) => {
          e.preventDefault();
          backend.scan();
        }}
      >
        <label class="field grow">
          <span>{t("scan.name")}</span>
          <input
            class="input"
            type="text"
            maxlength="24"
            autocomplete="off"
            placeholder={t("scan.namePlaceholder")}
            value={config.scanName}
            oninput={(e) => updateConfig({ scanName: e.currentTarget.value })}
          />
        </label>
        <button type="submit" class="btn primary" disabled={scanner.scanning}>
          {scanner.scanning ? t("scan.running") : t("scan.run")}
        </button>
      </form>

      <div class="result" aria-live="polite">
        {#if scanner.scanning}
          <p class="percent num scanning">…</p>
          <p class="verdict muted">{t("scan.running")}</p>
        {:else if latest}
          <p class="percent num">{latest.percent}<span>%</span></p>
          <p class="verdict">{latest.name}: {latest.verdict}</p>
        {:else}
          <p class="percent num idle">–</p>
          <p class="verdict muted">{t("scan.none")}</p>
        {/if}
      </div>
    </section>

    <section class="panel trigger" aria-labelledby="trigger-title">
      <h2 id="trigger-title">{t("scan.trigger")}</h2>
      <p class="small muted">{t("scan.triggerHelp")}</p>
      <Switch
        checked={config.scanParamEnabled}
        label={t("scan.triggerToggle")}
        onchange={(scanParamEnabled) => updateConfig({ scanParamEnabled })}
      />
      <label class="field">
        <span>{t("scan.param")}</span>
        <input
          class="input"
          type="text"
          spellcheck="false"
          autocomplete="off"
          placeholder={DEFAULT_SCAN_PARAM}
          value={config.scanParam}
          onchange={(e) => setParam(e.currentTarget)}
        />
      </label>
      {#if config.scanParamEnabled && !receiver.enabled}
        <div class="hint">
          <p class="small">{t("scan.needsReceive")}</p>
          <button type="button" class="btn" onclick={() => updateConfig({ oscReceive: true })}>
            {t("scan.enableReceive")}
          </button>
        </div>
      {:else if config.scanParamEnabled && receiver.error}
        <p class="small warn">{t("osc.portError", { port: receiver.port, error: receiver.error })}</p>
      {/if}
    </section>
  </div>

  <section class="panel" aria-labelledby="results-title">
    <h2 id="results-title">{t("scan.results")}</h2>
    {#if scanner.results.length}
      <ol class="results">
        {#each scanner.results as r}
          <li>
            <span class="small muted num">{r.time}</span>
            <span class="who">{r.name}</span>
            <b class="num">{r.percent} %</b>
            <span class="small muted what">{r.verdict}</span>
          </li>
        {/each}
      </ol>
    {:else}
      <p class="small muted empty">{t("scan.empty")}</p>
    {/if}
  </section>
</div>

<style>
  .columns {
    display: grid;
    grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr);
    gap: 20px;
  }

  .scan,
  .trigger {
    display: grid;
    gap: 14px;
    align-content: start;
  }

  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }

  form {
    display: flex;
    align-items: end;
    gap: 10px;
  }

  .grow {
    flex: 1;
  }

  .result {
    display: grid;
    justify-items: center;
    gap: 4px;
    padding: 18px 12px 8px;
    text-align: center;
  }

  .percent {
    font: 600 72px/1 var(--display);
    letter-spacing: -0.04em;
    color: var(--pulse);
  }

  .percent span {
    font-size: 0.45em;
    margin-left: 4px;
  }

  .percent.idle {
    color: var(--line);
  }

  .percent.scanning {
    animation: blink 900ms ease-in-out infinite alternate;
  }

  @keyframes blink {
    from {
      opacity: 1;
    }
    to {
      opacity: 0.35;
    }
  }

  .verdict {
    min-height: 1.5em;
    font-size: 16px;
    user-select: text;
  }

  .hint {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 14px;
    border-radius: 12px;
    border: 1px solid var(--warn);
    color: var(--warn);
  }

  .warn {
    color: var(--warn);
  }

  .results {
    display: grid;
    margin: 12px 0 0;
    padding: 0;
    list-style: none;
    user-select: text;
  }

  .results li {
    display: grid;
    grid-template-columns: 72px minmax(80px, 160px) 56px 1fr;
    align-items: baseline;
    gap: 12px;
    padding: 9px 0;
    border-top: 1px solid var(--line);
  }

  .who {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .results b {
    color: var(--pulse);
    text-align: right;
  }

  .empty {
    margin-top: 10px;
  }

  @media (max-width: 880px) {
    .columns {
      grid-template-columns: 1fr;
    }
  }
</style>
