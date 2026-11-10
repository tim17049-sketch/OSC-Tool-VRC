<script lang="ts">
  import { app, updateConfig } from "../lib/state.svelte";
  import { t } from "../lib/i18n";
  import type { Config, Theme } from "../lib/types";
  import { ACCENTS, PALETTES } from "../lib/theme";

  const THEMES: Theme[] = ["night", "dark", "amoled", "light"];

  const config = $derived(app.config!);

  function setPort(key: "oscPort" | "hrPort", input: HTMLInputElement) {
    const value = Math.round(input.valueAsNumber);
    if (value >= 1 && value <= 65535) updateConfig({ [key]: value });
    else input.value = String(config[key]);
  }

  function setHost(input: HTMLInputElement) {
    const value = input.value.trim();
    if (value) updateConfig({ oscHost: value });
    else input.value = config.oscHost;
  }
</script>

<div class="page">
  <h1>{t("nav.settings")}</h1>

  <section class="panel" aria-labelledby="set-vrchat">
    <h2 id="set-vrchat">{t("set.vrchat")}</h2>
    <div class="fields">
      <label class="field grow">
        <span>{t("set.host")}</span>
        <input
          class="input num"
          type="text"
          spellcheck="false"
          autocomplete="off"
          value={config.oscHost}
          onchange={(e) => setHost(e.currentTarget)}
        />
      </label>
      <label class="field port">
        <span>{t("set.port")}</span>
        <input
          class="input num"
          type="number"
          min="1"
          max="65535"
          value={config.oscPort}
          onchange={(e) => setPort("oscPort", e.currentTarget)}
        />
      </label>
    </div>
    <p class="small muted">{t("set.vrchatHelp")}</p>
  </section>

  <section class="panel" aria-labelledby="set-watch">
    <h2 id="set-watch">{t("set.watch")}</h2>
    <div class="fields">
      <label class="field port">
        <span>{t("set.hrPort")}</span>
        <input
          class="input num"
          type="number"
          min="1"
          max="65535"
          value={config.hrPort}
          onchange={(e) => setPort("hrPort", e.currentTarget)}
        />
      </label>
      <label class="field grow">
        <span class="row">
          {t("set.hrTimeout")}
          <b class="num">{t("tpl.seconds", { s: config.hrTimeoutS })}</b>
        </span>
        <input
          type="range"
          min="3"
          max="60"
          step="1"
          value={config.hrTimeoutS}
          oninput={(e) => updateConfig({ hrTimeoutS: e.currentTarget.valueAsNumber })}
        />
      </label>
    </div>
  </section>

  <section class="panel" aria-labelledby="set-look">
    <h2 id="set-look">{t("look.title")}</h2>
    <div class="field">
      <span id="theme-label">{t("look.theme")}</span>
      <div class="themes" role="radiogroup" aria-labelledby="theme-label">
        {#each THEMES as theme}
          {@const p = PALETTES[theme]}
          <button
            type="button"
            role="radio"
            class="theme"
            aria-checked={config.theme === theme}
            onclick={() => updateConfig({ theme })}
          >
            <span class="swatch" style:background={p.ink} style:border-color={p.line}>
              <span style:background={p.panel}></span>
              <span style:background={config.accent || p.accent}></span>
            </span>
            {t(`look.${theme}`)}
          </button>
        {/each}
      </div>
    </div>
    <div class="field">
      <span id="accent-label">{t("look.accent")}</span>
      <div class="accents" role="radiogroup" aria-labelledby="accent-label">
        {#each ACCENTS as color}
          <button
            type="button"
            role="radio"
            class="accent"
            aria-checked={config.accent.toLowerCase() === color}
            aria-label={color}
            title={color}
            style:background={color}
            onclick={() => updateConfig({ accent: color })}
          ></button>
        {/each}
        <label class="custom" title={t("look.custom")}>
          <input
            type="color"
            aria-label={t("look.custom")}
            value={config.accent || PALETTES[config.theme].accent}
            oninput={(e) => updateConfig({ accent: e.currentTarget.value })}
          />
        </label>
        <button
          type="button"
          class="btn quiet"
          disabled={config.accent === ""}
          onclick={() => updateConfig({ accent: "" })}
        >
          {t("look.reset")}
        </button>
      </div>
    </div>
  </section>

  <section class="panel" aria-labelledby="set-app">
    <h2 id="set-app">{t("set.app")}</h2>
    <div class="fields">
      <label class="field language">
        <span>{t("set.language")}</span>
        <select
          class="input"
          value={config.language}
          onchange={(e) => updateConfig({ language: e.currentTarget.value as Config["language"] })}
        >
          <option value="auto">{t("set.language.auto")}</option>
          <option value="de">Deutsch</option>
          <option value="en">English</option>
        </select>
      </label>
    </div>
    <div class="safe">
      <h3>{t("set.safe")}</h3>
      <p class="small muted">{t("set.safeText")}</p>
    </div>
    <p class="small muted">{t("set.about", { v: __APP_VERSION__ })}</p>
  </section>
</div>

<style>
  .page {
    max-width: 720px;
  }

  section {
    display: grid;
    gap: 14px;
  }

  .fields {
    display: flex;
    gap: 16px;
    align-items: start;
  }

  .grow {
    flex: 1;
  }

  .port {
    width: 150px;
  }

  .language {
    width: 240px;
  }

  .row {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }

  .row b {
    color: var(--text);
  }

  .themes {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px;
  }

  .theme {
    display: grid;
    gap: 8px;
    padding: 8px 8px 10px;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: none;
    font-weight: 600;
    cursor: pointer;
    transition: border-color 120ms ease-out;
  }

  .theme:hover {
    border-color: var(--line-hover);
  }

  .theme[aria-checked="true"] {
    border-color: var(--pulse);
    box-shadow: 0 0 0 1px var(--pulse);
  }

  .swatch {
    display: flex;
    align-items: end;
    gap: 6px;
    height: 48px;
    padding: 8px;
    border: 1px solid;
    border-radius: 8px;
  }

  .swatch span:first-child {
    flex: 1;
    height: 100%;
    border-radius: 5px;
  }

  .swatch span:last-child {
    width: 18px;
    height: 18px;
    border-radius: 50%;
  }

  .accents {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }

  .accent,
  .custom {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    border: 2px solid transparent;
    cursor: pointer;
  }

  .accent[aria-checked="true"] {
    box-shadow:
      0 0 0 2px var(--panel),
      0 0 0 4px var(--text);
  }

  .custom {
    position: relative;
    overflow: hidden;
    background: conic-gradient(#ff5c8a, #facc15, #4ade80, #22d3ee, #a78bfa, #ff5c8a);
  }

  .custom input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
  }

  .custom:focus-within {
    outline: 2px solid var(--signal);
    outline-offset: 2px;
  }

  .safe {
    display: grid;
    gap: 4px;
    padding: 14px 16px;
    border-radius: 12px;
    background: var(--raised);
    user-select: text;
  }

  h3 {
    font-size: 14px;
    font-weight: 600;
    color: var(--signal);
  }
</style>
