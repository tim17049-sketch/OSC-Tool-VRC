<script lang="ts">
  import { app, updateConfig } from "../lib/state.svelte";
  import { num, t, type Key } from "../lib/i18n";
  import {
    CHATBOX_MAX_CHARS,
    CHATBOX_MAX_LINES,
    DEFAULT_TEMPLATE,
    MIN_INTERVAL_MS,
  } from "../lib/types";
  import Bubble from "../components/Bubble.svelte";
  import Switch from "../components/Switch.svelte";

  const config = $derived(app.config!);
  const snap = $derived(app.snap!);

  const groups: { label: Key; names: string[] }[] = [
    { label: "tpl.group.pulse", names: ["hr", "hr_trend", "heart", "hr_min", "hr_max", "hr_avg"] },
    { label: "tpl.group.music", names: ["song", "title", "artist"] },
    { label: "tpl.group.time", names: ["time", "date"] },
    { label: "tpl.group.system", names: ["cpu", "cpu_temp", "ram", "gpu", "gpu_temp"] },
    { label: "tpl.group.vr", names: ["hmd_bat", "left_bat", "right_bat"] },
  ];

  let editor: HTMLTextAreaElement;

  // The engine cuts to VRChat's limits; count what the template asks for.
  const preview = $derived(snap.chatbox.messageRemainingS === null ? snap.chatbox.preview : "");
  const chars = $derived([...preview].reduce((n, c) => n + (c.codePointAt(0)! > 0xffff ? 2 : 1), 0));
  const lines = $derived(preview ? preview.split("\n").length : 0);
  const clipped = $derived(preview.endsWith("…") && chars >= CHATBOX_MAX_CHARS);
  const tooManyLines = $derived(config.template.split("\n").length > CHATBOX_MAX_LINES);

  function insert(name: string) {
    const token = `{${name}}`;
    const start = editor.selectionStart ?? config.template.length;
    const end = editor.selectionEnd ?? start;
    updateConfig({ template: config.template.slice(0, start) + token + config.template.slice(end) });
    requestAnimationFrame(() => {
      editor.focus();
      editor.setSelectionRange(start + token.length, start + token.length);
    });
  }
</script>

<div class="page">
  <h1>{t("nav.chatbox")}</h1>

  <div class="columns">
    <section class="panel editor" aria-labelledby="tpl-title">
      <header>
        <h2 id="tpl-title">{t("tpl.title")}</h2>
        <button
          type="button"
          class="btn quiet"
          disabled={config.template === DEFAULT_TEMPLATE}
          onclick={() => updateConfig({ template: DEFAULT_TEMPLATE })}
        >
          {t("tpl.reset")}
        </button>
      </header>

      <label class="field">
        <span>{t("tpl.label")}</span>
        <textarea
          class="input"
          rows="5"
          spellcheck="false"
          bind:this={editor}
          value={config.template}
          oninput={(e) => updateConfig({ template: e.currentTarget.value })}
        ></textarea>
      </label>
      <p class="small muted">{t("tpl.help")}</p>

      <div class="chips" role="group" aria-label={t("tpl.insert")}>
        <p class="small muted">{t("tpl.insert")}</p>
        {#each groups as group}
          <div class="group">
            <span class="small muted name">{t(group.label)}</span>
            <div class="set">
              {#each group.names as name}
                <button type="button" class="chip" onclick={() => insert(name)}>
                  <code>{`{${name}}`}</code>
                  <span>{t(`ph.${name}` as Key)}</span>
                </button>
              {/each}
            </div>
          </div>
        {/each}
      </div>
    </section>

    <div class="side">
      <section class="panel" aria-labelledby="preview-title">
        <h2 id="preview-title">{t("tpl.preview")}</h2>
        <div class="preview">
          {#if preview}
            <Bubble text={preview} dim={!config.chatboxEnabled} />
          {:else if snap.chatbox.messageRemainingS !== null}
            <Bubble text={snap.chatbox.preview} />
          {:else}
            <p class="muted small">{t("stage.empty")}</p>
          {/if}
        </div>
        <p class="small muted num counts">
          <span class:over={clipped}>{t("tpl.chars", { n: chars, max: CHATBOX_MAX_CHARS })}</span>
          <span class:over={tooManyLines}>{t("tpl.lines", { n: lines, max: CHATBOX_MAX_LINES })}</span>
        </p>
        {#if clipped || tooManyLines}
          <p class="small over">{t("tpl.tooLong")}</p>
        {/if}
      </section>

      <section class="panel behaviour" aria-labelledby="behaviour-title">
        <h2 id="behaviour-title">{t("tpl.behaviour")}</h2>

        <label class="field">
          <span class="row">
            {t("tpl.interval")}
            <b class="num">{t("tpl.seconds", { s: num(config.intervalMs / 1000) })}</b>
          </span>
          <input
            type="range"
            min={MIN_INTERVAL_MS}
            max="10000"
            step="500"
            value={config.intervalMs}
            oninput={(e) => updateConfig({ intervalMs: e.currentTarget.valueAsNumber })}
          />
        </label>
        <p class="small muted">{t("tpl.intervalHelp")}</p>

        <label class="field">
          <span class="row">
            {t("tpl.messageSeconds")}
            <b class="num">{t("tpl.seconds", { s: config.messageSeconds })}</b>
          </span>
          <input
            type="range"
            min="4"
            max="60"
            step="2"
            value={config.messageSeconds}
            oninput={(e) => updateConfig({ messageSeconds: e.currentTarget.valueAsNumber })}
          />
        </label>

        <Switch
          checked={config.time24h}
          label={t("tpl.time24")}
          onchange={(time24h) => updateConfig({ time24h })}
        />
        <Switch
          checked={config.notifySound}
          label={t("tpl.sound")}
          onchange={(notifySound) => updateConfig({ notifySound })}
        />
      </section>
    </div>
  </div>
</div>

<style>
  .columns {
    display: grid;
    grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr);
    gap: 20px;
    align-items: start;
  }

  .side,
  .editor,
  .behaviour {
    display: grid;
    gap: 14px;
  }

  .side {
    gap: 20px;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  header .btn {
    height: 32px;
  }

  textarea {
    font-size: 16px;
  }

  .chips {
    display: grid;
    gap: 10px;
    padding-top: 6px;
    border-top: 1px solid var(--line);
  }

  .chips > p {
    margin-top: 8px;
  }

  .group {
    display: grid;
    grid-template-columns: 52px 1fr;
    gap: 8px;
  }

  .name {
    line-height: 32px;
  }

  .set {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-height: 32px;
    padding: 0 12px 0 10px;
    border-radius: 999px;
    border: 1px solid var(--line);
    background: var(--raised);
    font-size: 13px;
    cursor: pointer;
    transition: border-color 120ms ease-out;
  }

  .chip:hover {
    border-color: var(--pulse);
  }

  code {
    font: 600 13px/1 var(--body);
    color: var(--pulse);
  }

  .preview {
    display: grid;
    place-items: center;
    min-height: 150px;
    margin: 14px 0 12px;
    padding: 20px 12px;
    border-radius: 12px;
    background: linear-gradient(180deg, #100e22 0%, #1a1636 100%);
  }

  .counts {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }

  .over {
    color: var(--warn);
  }

  .row {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }

  .row b {
    color: var(--text);
  }

  @media (max-width: 880px) {
    .columns {
      grid-template-columns: 1fr;
    }
  }
</style>
