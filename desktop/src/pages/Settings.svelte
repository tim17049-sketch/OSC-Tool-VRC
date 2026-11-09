<script lang="ts">
  import { app, updateConfig } from "../lib/state.svelte";
  import { t } from "../lib/i18n";
  import type { Config } from "../lib/types";

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
