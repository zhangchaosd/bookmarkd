<script lang="ts">
  export let name: 'copy' | 'edit' | 'pin' | 'up' | 'trash' | 'restore';
  export let filled = false;
</script>

<svg
  width="14"
  height="14"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.6"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  {#if name === 'copy'}
    <rect x="8" y="8" width="12" height="12" rx="2" />
    <path d="M16 8V4H4v12h4" />
  {:else if name === 'edit'}
    <path d="m15 4 5 5M4 20l5-1L21 7l-5-5L4 14v6Z" />
  {:else if name === 'pin'}
    <path
      fill={filled ? 'currentColor' : 'none'}
      d="m12 3 2.8 5.7 6.2.9-4.5 4.4 1.1 6.2L12 17.3l-5.6 2.9 1.1-6.2L3 9.6l6.2-.9Z"
    />
  {:else if name === 'up'}
    <path d="M12 20V4m-6 6 6-6 6 6" />
  {:else if name === 'trash'}
    <path d="M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7m4-7v7" />
  {:else}
    <path d="M4 4v6h6M4 10a8 8 0 1 1 0 6" />
  {/if}
</svg>
