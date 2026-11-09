<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { t } from "../lib/i18n";

  const W = 600;
  const H = 150;
  const SPAN = 5 * 60 * 1000;

  // Re-evaluates with every snapshot, which is what moves the chart along.
  const chart = $derived.by(() => {
    const now = Date.now();
    const points = app.history;
    void app.snap;
    if (points.length < 2) return null;
    const values = points.map((p) => p.bpm);
    const low = Math.floor((Math.min(...values) - 5) / 10) * 10;
    const high = Math.ceil((Math.max(...values) + 5) / 10) * 10;
    const x = (at: number) => W - ((now - at) / SPAN) * W;
    const y = (bpm: number) => H - ((bpm - low) / (high - low)) * H;
    const line = points.map((p) => `${x(p.at).toFixed(1)},${y(p.bpm).toFixed(1)}`).join(" ");
    const first = points[0];
    const last = points[points.length - 1];
    return {
      low,
      high,
      line,
      area: `${x(first.at).toFixed(1)},${H} ${line} ${x(last.at).toFixed(1)},${H}`,
      mid: (low + high) / 2,
    };
  });
</script>

{#if chart}
  <figure>
    <div class="plot">
      <div class="axis small muted num" aria-hidden="true">
        <span>{chart.high}</span>
        <span>{chart.mid}</span>
        <span>{chart.low}</span>
      </div>
      <svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" role="img" aria-label={t("history.title")}>
        <line x1="0" x2={W} y1={H / 2} y2={H / 2} class="grid" />
        <polygon points={chart.area} class="area" />
        <polyline points={chart.line} class="line" />
      </svg>
    </div>
    <figcaption class="small muted">
      <span>{t("history.ago")}</span>
      <span>{t("history.now")}</span>
    </figcaption>
  </figure>
{:else}
  <p class="muted empty">{t("history.empty")}</p>
{/if}

<style>
  figure {
    margin: 0;
    display: grid;
    gap: 6px;
  }

  .plot {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 10px;
    height: 150px;
  }

  .axis {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    text-align: right;
    line-height: 1;
  }

  svg {
    width: 100%;
    height: 100%;
    border-bottom: 1px solid var(--line);
    border-top: 1px solid var(--line);
    overflow: visible;
  }

  .grid {
    stroke: var(--line);
    stroke-dasharray: 3 5;
    vector-effect: non-scaling-stroke;
  }

  .line {
    fill: none;
    stroke: var(--pulse);
    stroke-width: 2;
    stroke-linejoin: round;
    vector-effect: non-scaling-stroke;
  }

  .area {
    fill: rgba(255, 92, 138, 0.12);
  }

  figcaption {
    display: flex;
    justify-content: space-between;
    padding-left: 34px;
  }

  .empty {
    padding: 34px 0;
    text-align: center;
  }
</style>
