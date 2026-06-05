<script lang="ts">
  type Bucket = {
    label: string;
    bottom: number;
    top: number;
  };

  type Props = {
    data: Bucket[];
    height?: number;
    bottomColor?: string;
    topColor?: string;
    bottomLabel?: string;
    topLabel?: string;
    showEveryNthLabel?: number;
    unit?: string;
  };

  let {
    data,
    height = 220,
    bottomColor = "var(--tr-green)",
    topColor = "var(--tr-sun)",
    bottomLabel = "Eigenverbrauch",
    topLabel = "Einspeisung",
    showEveryNthLabel = 1,
    unit = "kWh",
  }: Props = $props();

  let max = $derived(
    data.length === 0 ? 0 : Math.max(0.1, ...data.map((d) => d.bottom + d.top)),
  );

  let yTicks = $derived([max, max / 2, 0]);

  function fmtAxis(v: number): string {
    if (v >= 1000) return `${(v / 1000).toFixed(1).replace(".", ",")}k`;
    return Math.round(v).toString();
  }

  function fmtTotal(v: number): string {
    return v.toLocaleString("de-DE", { maximumFractionDigits: 1 });
  }

  function pct(v: number): number {
    return max === 0 ? 0 : (v / max) * 100;
  }

  function showLabelAt(i: number, n: number): boolean {
    if (n <= 1) return true;
    return i % n === 0 || i === data.length - 1;
  }
</script>

{#if data.length === 0}
  <div class="px-5 py-10 text-center text-sm text-[var(--tr-text-dim)]">
    Keine Daten im Zeitraum.
  </div>
{:else}
  <div class="px-5 pb-4">
    <div class="flex gap-3" style:height={`${height}px`}>
      <div
        class="flex w-9 flex-col justify-between py-0.5 text-right font-mono text-[10px] text-[var(--tr-text-dim)]"
      >
        {#each yTicks as t (t)}
          <span>{fmtAxis(t)}</span>
        {/each}
      </div>
      <div
        class="relative flex flex-1 items-end gap-[2px] border-b border-l border-[var(--tr-line)]"
      >
        {#each yTicks as _, i (i)}
          <div
            class="pointer-events-none absolute left-0 right-0 border-t border-dashed border-[var(--tr-line)]"
            style:top={`${(i / (yTicks.length - 1)) * 100}%`}
            style:opacity={i === yTicks.length - 1 ? 0 : 0.4}
          ></div>
        {/each}
        {#each data as d, i (i)}
          <div
            class="group relative flex h-full flex-1 flex-col justify-end transition-opacity hover:opacity-90"
            title={`${d.label} — ${bottomLabel}: ${fmtTotal(d.bottom)} ${unit}, ${topLabel}: ${fmtTotal(d.top)} ${unit}, gesamt: ${fmtTotal(d.bottom + d.top)} ${unit}`}
          >
            {#if d.top > 0}
              <div
                style:background={topColor}
                style:height={`${pct(d.top)}%`}
              ></div>
            {/if}
            {#if d.bottom > 0}
              <div
                style:background={bottomColor}
                style:height={`${pct(d.bottom)}%`}
              ></div>
            {/if}
          </div>
        {/each}
      </div>
    </div>

    <div class="mt-1 flex gap-[2px] pl-12">
      {#each data as d, i (i)}
        <div
          class="flex-1 truncate text-center font-mono text-[10px] text-[var(--tr-text-dim)]"
        >
          {#if showLabelAt(i, showEveryNthLabel)}{d.label}{/if}
        </div>
      {/each}
    </div>

    <div class="mt-3 flex flex-wrap gap-4 text-xs text-[var(--tr-text-dim)]">
      <span class="inline-flex items-center gap-1.5">
        <span
          class="inline-block size-3 rounded-sm"
          style:background={bottomColor}
        ></span>
        {bottomLabel}
      </span>
      <span class="inline-flex items-center gap-1.5">
        <span
          class="inline-block size-3 rounded-sm"
          style:background={topColor}
        ></span>
        {topLabel}
      </span>
    </div>
  </div>
{/if}
