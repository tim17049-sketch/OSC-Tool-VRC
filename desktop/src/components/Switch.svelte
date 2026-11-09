<script lang="ts">
  let {
    checked,
    label,
    onchange,
  }: { checked: boolean; label: string; onchange: (checked: boolean) => void } = $props();
</script>

<button
  type="button"
  class="switch"
  role="switch"
  aria-checked={checked}
  onclick={() => onchange(!checked)}
>
  <span class="track"><span class="thumb"></span></span>
  <span>{label}</span>
</button>

<style>
  .switch {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    min-height: 40px;
    padding: 0;
    background: none;
    border: 0;
    border-radius: var(--r-control);
    font-weight: 600;
    cursor: pointer;
    text-align: left;
  }

  .track {
    width: 42px;
    height: 24px;
    padding: 3px;
    border-radius: 999px;
    background: var(--line);
    flex: none;
    transition: background 140ms ease-out;
  }

  .thumb {
    display: block;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--muted);
    transition: transform 140ms ease-out, background 140ms ease-out;
  }

  .switch[aria-checked="true"] .track {
    background: var(--pulse);
  }

  .switch[aria-checked="true"] .thumb {
    background: #fff;
    transform: translateX(18px);
  }
</style>
