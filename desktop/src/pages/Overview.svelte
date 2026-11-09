<script lang="ts">
  import { app, updateConfig } from "../lib/state.svelte";
  import { backend } from "../lib/backend";
  import { num, t } from "../lib/i18n";
  import { CHATBOX_MAX_CHARS } from "../lib/types";
  import Bubble from "../components/Bubble.svelte";
  import Ecg from "../components/Ecg.svelte";
  import Switch from "../components/Switch.svelte";

  const config = $derived(app.config!);
  const snap = $derived(app.snap!);
  const hr = $derived(snap.hr);
  const chat = $derived(snap.chatbox);
  const target = $derived(`${config.oscHost}:${config.oscPort}`);
  const showing = $derived(chat.messageRemainingS !== null);
  const visible = $derived(chat.preview !== "" && (chat.enabled || showing));
  const device = $derived(hr.device ?? t("pulse.unknownDevice"));

  let draft = $state("");
  let typing = false;

  function setTyping(next: boolean) {
    if (next === typing) return;
    typing = next;
    backend.setTyping(next);
  }

  function send(event: SubmitEvent) {
    event.preventDefault();
    if (!draft.trim()) return;
    backend.sendMessage(draft);
    draft = "";
    setTyping(false);
  }
</script>

<div class="page">
  <div class="top">
    <section class="panel stage" aria-labelledby="stage-title">
      <header>
        <h2 id="stage-title">{t("stage.title")}</h2>
        <Switch
          checked={config.chatboxEnabled}
          label={t("stage.sending")}
          onchange={(chatboxEnabled) => updateConfig({ chatboxEnabled })}
        />
      </header>

      <div class="scene">
        {#if chat.preview}
          <Bubble text={chat.preview} dim={!visible} />
        {:else}
          <div class="empty">
            <p class="muted">{t("stage.empty")}</p>
            <button type="button" class="btn" onclick={() => (app.page = "chatbox")}>
              {t("stage.emptyAction")}
            </button>
          </div>
        {/if}
        <div class="avatar" aria-hidden="true">
          <span class="plate">{t("stage.you")}</span>
          <svg viewBox="0 0 200 96" width="200" height="96">
            <circle cx="100" cy="34" r="26" />
            <path d="M28 96c4-22 28-32 72-32s68 10 72 32z" />
          </svg>
        </div>
      </div>

      <p class="small muted status" class:warn={chat.error}>
        {#if chat.error}
          {t("stage.oscError", { target, error: chat.error })}
        {:else if !chat.enabled && !showing}
          {t("stage.off")}
        {:else}
          {t("stage.target", { s: num(config.intervalMs / 1000), target })}
        {/if}
      </p>
    </section>

    <section class="panel pulse" aria-labelledby="pulse-title">
      <h2 id="pulse-title">{t("pulse.title")}</h2>

      <div class="reading" class:idle={hr.bpm === null}>
        <span class="bpm num">{hr.bpm ?? "--"}</span>
        <span class="unit">
          <span class="muted">{t("pulse.bpm")}</span>
          {#if hr.bpm !== null && hr.trend !== null}
            <span class="trend">
              {hr.trend > 0 ? "↑" : hr.trend < 0 ? "↓" : "→"}
              {t(hr.trend > 0 ? "pulse.rising" : hr.trend < 0 ? "pulse.falling" : "pulse.steady")}
            </span>
          {/if}
        </span>
      </div>

      <div class="trace"><Ecg bpm={hr.bpm} /></div>

      <dl class="range">
        <div><dt>{t("pulse.min")}</dt><dd class="num">{hr.min ?? "–"}</dd></div>
        <div><dt>{t("pulse.avg")}</dt><dd class="num">{hr.avg ?? "–"}</dd></div>
        <div><dt>{t("pulse.max")}</dt><dd class="num">{hr.max ?? "–"}</dd></div>
      </dl>

      <div class="watch small">
        {#if hr.bpm !== null}
          <span class="dot live"></span>
          <span>{t("pulse.connected", { device })}</span>
        {:else if hr.connected}
          <span class="dot warn"></span>
          <span>{t("pulse.noContact", { device })}</span>
        {:else}
          <span class="dot"></span>
          <span class="muted">{t("pulse.waiting")}</span>
          <button type="button" class="btn quiet" onclick={() => (app.page = "pulse")}>
            {t("pulse.waitingAction")}
          </button>
        {/if}
      </div>
    </section>
  </div>

  <section class="panel message" aria-labelledby="msg-title">
    <h2 id="msg-title">{t("msg.title")}</h2>
    <form onsubmit={send}>
      <div class="entry">
        <input
          class="input"
          type="text"
          aria-labelledby="msg-title"
          placeholder={t("msg.placeholder")}
          maxlength={CHATBOX_MAX_CHARS}
          autocomplete="off"
          bind:value={draft}
          oninput={() => setTyping(draft.trim() !== "")}
          onblur={() => setTyping(false)}
        />
        <span class="count small muted num" aria-hidden="true">{draft.length}/{CHATBOX_MAX_CHARS}</span>
      </div>
      <button type="submit" class="btn primary">{t("msg.send")}</button>
    </form>
    {#if showing}
      <p class="small note">
        <span>{t("msg.showing", { s: chat.messageRemainingS ?? 0 })}</span>
        <button type="button" class="btn quiet" onclick={() => backend.sendMessage("")}>
          {t("msg.stop")}
        </button>
      </p>
    {/if}
  </section>
</div>

<style>
  .top {
    display: grid;
    grid-template-columns: minmax(0, 1.15fr) minmax(0, 1fr);
    gap: 20px;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  /* ---- stage ---- */
  .stage {
    display: grid;
    grid-template-rows: auto 1fr auto;
    gap: 14px;
    padding-bottom: 18px;
  }

  .scene {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: flex-end;
    gap: 14px;
    min-height: 280px;
    padding: 24px 16px 0;
    border-radius: 12px;
    overflow: hidden;
    background:
      radial-gradient(120% 70% at 50% 100%, rgba(255, 92, 138, 0.16), transparent 70%),
      linear-gradient(180deg, #100e22 0%, #1a1636 100%);
  }

  .avatar {
    display: grid;
    justify-items: center;
    gap: 12px;
    flex: none;
  }

  .avatar svg {
    display: block;
    fill: #322b5c;
  }

  .plate {
    padding: 3px 14px;
    border-radius: 6px;
    background: rgba(12, 10, 24, 0.7);
    font-size: 13px;
    font-weight: 600;
    color: var(--muted);
  }

  .empty {
    display: grid;
    justify-items: center;
    gap: 12px;
    text-align: center;
    margin-bottom: 8px;
  }

  .status {
    user-select: text;
  }

  .status.warn {
    color: var(--warn);
  }

  /* ---- pulse ---- */
  .pulse {
    display: grid;
    grid-template-rows: auto auto 1fr auto auto;
    gap: 14px;
    padding-bottom: 18px;
  }

  .reading {
    display: flex;
    align-items: flex-end;
    gap: 14px;
    height: 78px;
    margin-top: 6px;
  }

  .unit {
    padding-bottom: 4px;
  }

  .bpm {
    font: 600 84px/0.9 var(--display);
    letter-spacing: -0.04em;
    color: var(--pulse);
  }

  .reading.idle .bpm {
    color: var(--line);
  }

  .unit {
    display: grid;
    gap: 2px;
    font-size: 13px;
  }

  .trend {
    font-weight: 600;
  }

  .trace {
    min-height: 96px;
    border-radius: 10px;
    /* ECG paper */
    background:
      linear-gradient(rgba(255, 92, 138, 0.07) 1px, transparent 1px) 0 0 / 100% 16px,
      linear-gradient(90deg, rgba(255, 92, 138, 0.07) 1px, transparent 1px) 0 0 / 16px 100%,
      #191530;
    overflow: hidden;
  }

  .range {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
    margin: 0;
  }

  .range div {
    display: grid;
    gap: 1px;
  }

  dt {
    font-size: 13px;
    color: var(--muted);
  }

  dd {
    margin: 0;
    font: 500 20px/1.2 var(--display);
  }

  .watch {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 40px;
  }

  .watch .btn {
    margin-left: auto;
    height: 34px;
  }

  /* ---- message ---- */
  .message {
    display: grid;
    gap: 12px;
  }

  form {
    display: flex;
    gap: 10px;
  }

  .entry {
    position: relative;
    flex: 1;
  }

  .entry .input {
    padding-right: 76px;
  }

  .count {
    position: absolute;
    right: 12px;
    top: 10px;
    pointer-events: none;
  }

  .note {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--signal);
  }

  .note .btn {
    height: 32px;
  }

  @media (max-width: 880px) {
    .top {
      grid-template-columns: 1fr;
    }
  }
</style>
