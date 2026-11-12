<script lang="ts">
  /** The chatbox as it floats above an avatar in VRChat. */
  let { text, dim = false }: { text: string; dim?: boolean } = $props();
</script>

<div class="bubble" class:dim>
  {#each text.split("\n") as line}
    <span>{line || " "}</span>
  {/each}
</div>

<style>
  .bubble {
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    max-width: 320px;
    padding: 12px 20px;
    border-radius: 18px;
    background: rgba(12, 10, 24, 0.86);
    /* Always the in-game look, whatever the app theme. */
    color: #f1edfa;
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow: 0 18px 40px -18px rgba(0, 0, 0, 0.8);
    font-size: 17px;
    line-height: 1.4;
    text-align: center;
    overflow-wrap: anywhere;
    user-select: text;
    transition: opacity 160ms ease-out;
  }

  .bubble.dim {
    opacity: 0.4;
  }
</style>
