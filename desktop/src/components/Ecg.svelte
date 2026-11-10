<script lang="ts">
  import { onMount } from "svelte";

  /** A heart monitor trace that beats at the given rate; flat without one. */
  let { bpm }: { bpm: number | null } = $props();

  let canvas: HTMLCanvasElement;
  let current: number | null = null;
  let redraw = () => {};

  $effect(() => {
    current = bpm;
    redraw();
  });

  // One heartbeat, x in 0..1: P wave, QRS spike, T wave.
  function beat(x: number): number {
    const bump = (center: number, width: number, height: number) =>
      height * Math.exp(-((x - center) ** 2) / (2 * width * width));
    return (
      bump(0.16, 0.028, 0.1) -
      bump(0.262, 0.009, 0.14) +
      bump(0.29, 0.012, 1) -
      bump(0.322, 0.011, 0.24) +
      bump(0.5, 0.045, 0.24)
    );
  }

  onMount(() => {
    const ctx = canvas.getContext("2d")!;
    const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
    const SPEED = 120; // px per second
    let width = 0;
    let height = 0;
    let trace: number[] = [];
    let phase = 0;
    let carry = 0;
    let last = performance.now();
    let frame = 0;

    const paint = () => {
      const dpr = devicePixelRatio || 1;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, width, height);
      const base = height * 0.68;
      const amp = height * 0.56;
      // Follows the accent colour of the theme.
      const color = getComputedStyle(canvas).getPropertyValue("--pulse").trim() || "#ff5c8a";
      const fade = ctx.createLinearGradient(0, 0, width, 0);
      fade.addColorStop(0, color + "00");
      fade.addColorStop(0.35, color + "bf");
      fade.addColorStop(1, color);
      ctx.strokeStyle = fade;
      ctx.lineWidth = 2;
      ctx.lineJoin = "round";
      ctx.beginPath();
      trace.forEach((v, x) => (x ? ctx.lineTo(x, base - v * amp) : ctx.moveTo(x, base - v * amp)));
      ctx.stroke();
      const head = trace[trace.length - 1] ?? 0;
      ctx.fillStyle = color;
      ctx.beginPath();
      ctx.arc(width - 3, base - head * amp, 3, 0, Math.PI * 2);
      ctx.fill();
    };

    const fillStill = () => {
      const perBeat = 150;
      trace = Array.from({ length: Math.ceil(width) }, (_, x) =>
        current ? beat(((x % perBeat) + 0.5) / perBeat) : 0,
      );
      paint();
    };

    const resize = () => {
      const box = canvas.getBoundingClientRect();
      const dpr = devicePixelRatio || 1;
      width = box.width;
      height = box.height;
      canvas.width = Math.round(width * dpr);
      canvas.height = Math.round(height * dpr);
      const columns = Math.ceil(width);
      trace = trace.slice(-columns);
      while (trace.length < columns) trace.unshift(0);
      if (still) fillStill();
    };

    const tick = (now: number) => {
      const dt = Math.min(0.1, (now - last) / 1000);
      last = now;
      carry += dt * SPEED;
      const steps = Math.floor(carry);
      carry -= steps;
      for (let i = 0; i < steps; i++) {
        // Finish the running beat even if the signal drops, so it never jumps.
        if (current || phase > 0) {
          phase += (current ?? 60) / 60 / SPEED;
          if (phase >= 1) phase = current ? phase - 1 : 0;
        }
        trace.push(phase > 0 ? beat(phase) : 0);
        trace.shift();
      }
      paint();
      frame = requestAnimationFrame(tick);
    };

    const observer = new ResizeObserver(resize);
    observer.observe(canvas);
    resize();
    if (still) {
      redraw = fillStill;
    } else {
      frame = requestAnimationFrame(tick);
    }
    return () => {
      observer.disconnect();
      cancelAnimationFrame(frame);
      redraw = () => {};
    };
  });
</script>

<canvas bind:this={canvas} aria-hidden="true"></canvas>

<style>
  canvas {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
