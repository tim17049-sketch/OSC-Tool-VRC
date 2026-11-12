<script lang="ts">
  import { app, deviceName, updateConfig } from "../lib/state.svelte";
  import { t } from "../lib/i18n";
  import History from "../components/History.svelte";
  import Switch from "../components/Switch.svelte";

  const config = $derived(app.config!);
  const snap = $derived(app.snap!);
  const hr = $derived(snap.hr);
  const net = $derived(snap.network);
  const device = $derived(deviceName(hr.device, t("pulse.unknownDevice")));
  const info = $derived(hr.connected ? hr.device : null);
  const maker = $derived(
    info?.manufacturer ? info.manufacturer.charAt(0).toUpperCase() + info.manufacturer.slice(1) : null,
  );
  const details = $derived(
    info
      ? ([
          ["device.maker", maker],
          ["device.model", info.model],
          ["device.os", info.os],
          ["device.sensor", info.sensor],
          ["device.app", info.app],
        ] as const).filter(([, value]) => value)
      : [],
  );

  const percent = $derived(
    hr.bpm === null
      ? 0
      : Math.min(1, Math.max(0, (hr.bpm - config.hrMin) / (config.hrMax - config.hrMin))),
  );
  const params = $derived([
    ["HR", hr.bpm ?? 0],
    ["HRPercent", percent.toFixed(2)],
    ["FullHRPercent", (percent * 2 - 1).toFixed(2)],
    ["onesHR / tensHR / hundredsHR", [...String(hr.bpm ?? 0).padStart(3, "0")].reverse().join(" / ")],
    ["isHRConnected", hr.connected],
    ["isHRActive", hr.bpm !== null],
    ["isHRBeat", "♥"],
  ] as const);

  function setRange(key: "hrMin" | "hrMax", value: number) {
    if (!Number.isFinite(value)) return;
    const next = { hrMin: config.hrMin, hrMax: config.hrMax, [key]: Math.round(value) };
    if (next.hrMin >= 0 && next.hrMax <= 255 && next.hrMin < next.hrMax) updateConfig(next);
  }
</script>

<div class="page">
  <header class="title">
    <h1>{t("nav.pulse")}</h1>
    <p class="state small">
      {#if hr.bpm !== null}
        <span class="dot live"></span>{t("pulse.connected", { device })}
      {:else if hr.connected}
        <span class="dot warn"></span>{t("pulse.noContact", { device })}
      {:else if net.hrListening}
        <span class="dot"></span><span class="muted">{t("watch.listening")}</span>
      {/if}
    </p>
  </header>

  {#if net.hrError}
    <p class="banner" role="alert">
      {t("watch.portError", { port: net.hrPort, error: net.hrError })}
    </p>
  {/if}

  <div class="columns">
    {#if info}
      <section class="panel device" aria-labelledby="device-title">
        <h2 id="device-title">{t("device.title")}</h2>
        <p class="name">{device}</p>
        {#if info.battery !== null}
          <div class="battery" role="img" aria-label="{t('device.battery')} {info.battery} %">
            <div class="level" class:low={info.battery <= 15} style:width="{info.battery}%"></div>
          </div>
          <p class="small muted num">
            {t("pulse.battery", { n: info.battery })}{info.charging ? ` · ${t("pulse.charging")}` : ""}
          </p>
        {/if}
        {#if details.length}
          <dl>
            {#each details as [label, value]}
              <div><dt>{t(label)}</dt><dd>{value}</dd></div>
            {/each}
          </dl>
        {/if}
        {#if !info.app}
          <p class="small muted">{t("device.oldApp")}</p>
        {/if}
      </section>
    {:else}
    <section class="panel" aria-labelledby="watch-title">
      <h2 id="watch-title">{t("watch.title")}</h2>
      <ol>
        <li>{t("watch.step1")}</li>
        <li>{t("watch.step2")}</li>
        <li>{t("watch.step3")}</li>
      </ol>
      <div class="manual">
        <p class="small muted">{t("watch.manual")}</p>
        {#each net.addresses as address}
          <p class="address num">{address}<span class="muted">:{net.hrPort}</span></p>
        {:else}
          <p class="small warn">{t("watch.noAddress")}</p>
        {/each}
        <p class="small muted">{t("watch.firewall", { port: net.hrPort })}</p>
      </div>
    </section>
    {/if}

    <section class="panel" aria-labelledby="history-title">
      <h2 id="history-title">{t("history.title")}</h2>
      <div class="history"><History /></div>
    </section>
  </div>

  <section class="panel avatar" aria-labelledby="avatar-title">
    <div class="intro">
      <h2 id="avatar-title">{t("avatar.title")}</h2>
      <p class="small muted">{t("avatar.help")}</p>
      <Switch
        checked={config.hrAvatarParams}
        label={t("avatar.toggle")}
        onchange={(hrAvatarParams) => updateConfig({ hrAvatarParams })}
      />
      <fieldset>
        <legend class="small muted">{t("avatar.range")}</legend>
        <label>
          <span class="small muted">{t("avatar.from")}</span>
          <input
            class="input num"
            type="number"
            min="0"
            max="254"
            value={config.hrMin}
            onchange={(e) => setRange("hrMin", e.currentTarget.valueAsNumber)}
          />
        </label>
        <label>
          <span class="small muted">{t("avatar.to")}</span>
          <input
            class="input num"
            type="number"
            min="1"
            max="255"
            value={config.hrMax}
            onchange={(e) => setRange("hrMax", e.currentTarget.valueAsNumber)}
          />
        </label>
      </fieldset>
    </div>

    <table class:off={!config.hrAvatarParams}>
      <thead>
        <tr><th>{t("avatar.param")}</th><th>{t("avatar.value")}</th></tr>
      </thead>
      <tbody>
        {#each params as [name, value]}
          <tr>
            <td>{name}</td>
            <td class="num">
              {#if typeof value === "boolean"}
                <span class="dot" class:live={value}></span> {value}
              {:else}
                {value}
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>
</div>

<style>
  .title {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .state {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .banner {
    padding: 10px 14px;
    border-radius: var(--r-control);
    border: 1px solid var(--warn);
    color: var(--warn);
    user-select: text;
  }

  .columns {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.1fr);
    gap: 20px;
  }

  ol {
    margin: 14px 0 0;
    padding: 0;
    list-style: none;
    counter-reset: step;
    display: grid;
    gap: 10px;
  }

  li {
    counter-increment: step;
    display: grid;
    grid-template-columns: 26px 1fr;
    gap: 10px;
    align-items: start;
  }

  li::before {
    content: counter(step);
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    background: var(--raised);
    font: 500 12px/1 var(--display);
    color: var(--pulse);
  }

  .manual {
    display: grid;
    gap: 6px;
    margin-top: 16px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
  }

  .address {
    font: 500 20px/1.3 var(--display);
    user-select: text;
  }

  .warn {
    color: var(--warn);
  }

  .history {
    margin-top: 16px;
  }

  .device {
    display: grid;
    gap: 10px;
    align-content: start;
  }

  .name {
    margin-top: 4px;
    font: 500 24px/1.2 var(--display);
    letter-spacing: -0.01em;
    user-select: text;
  }

  .battery {
    height: 8px;
    border-radius: 999px;
    background: var(--raised);
    overflow: hidden;
  }

  .level {
    height: 100%;
    border-radius: 999px;
    background: var(--signal);
  }

  .level.low {
    background: var(--warn);
  }

  dl {
    display: grid;
    gap: 0;
    margin: 6px 0 0;
    user-select: text;
  }

  dl div {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    padding: 7px 0;
    border-top: 1px solid var(--line);
    font-size: 14px;
  }

  dl dt {
    color: var(--muted);
  }

  dl dd {
    margin: 0;
    text-align: right;
  }

  .avatar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.1fr);
    gap: 28px;
  }

  .intro {
    display: grid;
    gap: 12px;
    align-content: start;
  }

  fieldset {
    display: flex;
    align-items: center;
    gap: 14px;
    margin: 0;
    padding: 0;
    border: 0;
  }

  legend {
    padding: 0;
    margin-bottom: 6px;
    font-weight: 600;
  }

  fieldset label {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  fieldset .input {
    width: 84px;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 14px;
    align-self: start;
    user-select: text;
    transition: opacity 160ms ease-out;
  }

  table.off {
    opacity: 0.4;
  }

  th {
    text-align: left;
    font-size: 13px;
    font-weight: 600;
    color: var(--muted);
    padding: 0 0 8px;
  }

  td {
    padding: 7px 0;
    border-top: 1px solid var(--line);
  }

  th:last-child,
  td:last-child {
    text-align: right;
  }

  td .dot {
    margin-right: 6px;
  }

  @media (max-width: 880px) {
    .columns,
    .avatar {
      grid-template-columns: 1fr;
    }
  }
</style>
