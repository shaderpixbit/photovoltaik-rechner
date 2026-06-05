<script lang="ts">
  import { CalendarIcon, XIcon } from "@lucide/svelte";
  import { cn } from "$lib/utils";
  import Button from "./Button.svelte";

  type Props = {
    /** ISO YYYY-MM oder "" für leer. */
    value?: string;
    onchange?: (v: string) => void;
    placeholder?: string;
    disabled?: boolean;
    class?: string;
    id?: string;
  };

  let {
    value = $bindable(""),
    onchange,
    placeholder = "MM.JJJJ",
    disabled = false,
    class: className,
    id,
  }: Props = $props();

  const ISO_MONTH = /^\d{4}-\d{2}$/;
  const MONTHS_SHORT = [
    "Jan", "Feb", "Mär", "Apr", "Mai", "Jun",
    "Jul", "Aug", "Sep", "Okt", "Nov", "Dez",
  ];
  const MONTHS_LONG = [
    "Januar", "Februar", "März", "April", "Mai", "Juni",
    "Juli", "August", "September", "Oktober", "November", "Dezember",
  ];

  const now = new Date();
  const todayYear = now.getFullYear();
  const todayMonth = now.getMonth() + 1;
  const todayIso = `${todayYear}-${String(todayMonth).padStart(2, "0")}`;

  let open = $state(false);
  let draft = $state<string>(value);
  let cursorYear = $state(initialCursor(value));
  let wrapperEl = $state<HTMLDivElement | null>(null);

  function initialCursor(iso: string): number {
    if (ISO_MONTH.test(iso)) return Number(iso.slice(0, 4));
    return todayYear;
  }

  function pad2(n: number): string {
    return String(n).padStart(2, "0");
  }

  function formatLabel(iso: string): string {
    if (!ISO_MONTH.test(iso)) return iso;
    const [y, m] = iso.split("-").map(Number);
    return `${MONTHS_LONG[m - 1]} ${y}`;
  }

  function openPopover() {
    if (disabled) return;
    draft = value;
    cursorYear = initialCursor(value);
    open = true;
  }
  function cancel() {
    open = false;
  }
  function commit() {
    value = draft;
    onchange?.(draft);
    open = false;
  }
  function setThisMonth() {
    draft = todayIso;
    cursorYear = todayYear;
  }
  function clearDraft() {
    draft = "";
  }
  function prevYear() {
    cursorYear -= 1;
  }
  function nextYear() {
    cursorYear += 1;
  }
  function pickMonth(m: number) {
    draft = `${cursorYear}-${pad2(m)}`;
  }

  $effect(() => {
    if (!open) return;
    function onDoc(e: MouseEvent) {
      if (wrapperEl && !wrapperEl.contains(e.target as Node)) cancel();
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        cancel();
      } else if (e.key === "Enter") {
        e.preventDefault();
        commit();
      }
    }
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  });
</script>

<div bind:this={wrapperEl} class={cn("relative", className)}>
  <button
    type="button"
    {id}
    {disabled}
    class={cn(
      "flex h-9 w-full items-center justify-between gap-2 rounded-md border bg-[var(--tr-surface)] px-3 text-left text-sm",
      "border-[var(--tr-line)] hover:border-[var(--tr-line-hi)]",
      "focus:border-[var(--tr-sun)] focus:outline-none",
      "disabled:cursor-not-allowed disabled:opacity-50",
      open && "border-[var(--tr-sun)]",
    )}
    onclick={openPopover}
  >
    <span class={value ? "" : "text-[var(--tr-text-faint)]"}>
      {value ? formatLabel(value) : placeholder}
    </span>
    <CalendarIcon class="size-4 text-[var(--tr-text-faint)]" />
  </button>

  {#if open}
    <div
      class="absolute z-40 mt-1 w-72 rounded-md border p-3 shadow-lg"
      style="background: var(--tr-surface); border-color: var(--tr-line);"
    >
      <div class="mb-2 flex items-center justify-between">
        <button
          type="button"
          class="inline-flex h-7 w-7 items-center justify-center rounded hover:bg-[var(--tr-surface2)]"
          onclick={prevYear}
          aria-label="Vorheriges Jahr"
        >
          ‹
        </button>
        <div class="text-sm font-medium">{cursorYear}</div>
        <button
          type="button"
          class="inline-flex h-7 w-7 items-center justify-center rounded hover:bg-[var(--tr-surface2)]"
          onclick={nextYear}
          aria-label="Nächstes Jahr"
        >
          ›
        </button>
      </div>

      <div class="grid grid-cols-3 gap-1">
        {#each MONTHS_SHORT as label, i (i)}
          {@const iso = `${cursorYear}-${pad2(i + 1)}`}
          {@const isCurrent = iso === todayIso}
          {@const isSelected = iso === draft}
          <button
            type="button"
            class={cn(
              "h-9 rounded text-xs hover:bg-[var(--tr-surface2)]",
              isCurrent && !isSelected && "ring-1 ring-[var(--tr-sun)]",
              isSelected &&
                "bg-[var(--tr-sun)] text-black hover:bg-[var(--tr-sun)]",
            )}
            onclick={() => pickMonth(i + 1)}
          >
            {label}
          </button>
        {/each}
      </div>

      <div
        class="mt-3 flex items-center justify-between gap-2 border-t pt-3"
        style="border-color: var(--tr-line);"
      >
        <div class="flex gap-1">
          <Button size="sm" variant="ghost" onclick={setThisMonth}>
            Aktuell
          </Button>
          <Button size="sm" variant="ghost" onclick={clearDraft}>
            <XIcon class="size-3" />Leeren
          </Button>
        </div>
        <div class="flex gap-1">
          <Button size="sm" variant="ghost" onclick={cancel}>Abbrechen</Button>
          <Button size="sm" variant="primary" onclick={commit}>OK</Button>
        </div>
      </div>
    </div>
  {/if}
</div>
