<script lang="ts">
  import Icon from './Icon.svelte';
  type Item = {
    id: string;
    title: string;
    url_raw: string;
    notes: string;
    tags: string[];
    icon?: string | null;
  };
  type Cell = [number, number];
  export let items: Item[] = [];
  /** Saved cells by bookmark ID; cells beyond the current width are reflowed. */
  export let positions: Record<string, Cell> = {};
  export let newTab = true;
  export let onMove: (changed: Record<string, Cell>) => void = () => {};
  export let onEdit: (item: Item) => void = () => {};

  const CELL_W = 108,
    CELL_H = 116;
  let width = 0;
  $: cols = Math.max(2, Math.floor(width / CELL_W) || 6);
  $: placed = place(items, positions, cols);
  $: rows =
    Math.max(-1, ...[...placed.values()].map(([, r]) => r)) +
    (drag?.moved ? 2 : 1);

  // Saved cells win when they fit and are free; everything else takes the
  // first free cell in reading order, like icons on a resized desktop.
  function place(list: Item[], saved: Record<string, Cell>, columns: number) {
    const result = new Map<string, Cell>(),
      taken = new Set<string>();
    for (const item of list) {
      const cell = saved[item.id];
      if (
        Array.isArray(cell) &&
        cell[0] >= 0 &&
        cell[0] < columns &&
        cell[1] >= 0 &&
        !taken.has(cell.join())
      ) {
        result.set(item.id, [cell[0], cell[1]]);
        taken.add(cell.join());
      }
    }
    let next = 0;
    for (const item of list) {
      if (result.has(item.id)) continue;
      while (taken.has(`${next % columns},${Math.floor(next / columns)}`))
        next++;
      const cell: Cell = [next % columns, Math.floor(next / columns)];
      result.set(item.id, cell);
      taken.add(cell.join());
    }
    return result;
  }
  function occupant(cell: Cell, except: string) {
    for (const [id, c] of placed)
      if (id !== except && c[0] === cell[0] && c[1] === cell[1]) return id;
    return null;
  }
  // Moving onto an occupied cell swaps the two icons.
  function moveTo(id: string, target: Cell) {
    const from = placed.get(id);
    if (!from || (from[0] === target[0] && from[1] === target[1])) return;
    const other = occupant(target, id);
    const changed: Record<string, Cell> = { [id]: target };
    if (other) changed[other] = from;
    onMove(changed);
  }

  let board: HTMLElement;
  let drag: {
    id: string;
    pointer: number;
    startX: number;
    startY: number;
    grabX: number;
    grabY: number;
    x: number;
    y: number;
    moved: boolean;
  } | null = null;
  let suppressClick = false;
  $: target = drag?.moved ? cellAt(drag.x, drag.y) : null;
  function cellAt(x: number, y: number): Cell {
    return [
      Math.min(cols - 1, Math.max(0, Math.round(x / CELL_W))),
      Math.max(0, Math.round(y / CELL_H)),
    ];
  }
  function down(event: PointerEvent, id: string) {
    // Touch keeps native scrolling; rearranging needs a mouse, pen or keyboard.
    if (event.button !== 0 || event.pointerType === 'touch') return;
    const tile = event.currentTarget as HTMLElement;
    const rect = tile.getBoundingClientRect();
    const origin = board.getBoundingClientRect();
    drag = {
      id,
      pointer: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      grabX: event.clientX - rect.left,
      grabY: event.clientY - rect.top,
      x: rect.left - origin.left,
      y: rect.top - origin.top,
      moved: false,
    };
  }
  function move(event: PointerEvent) {
    if (!drag || event.pointerId !== drag.pointer) return;
    if (
      !drag.moved &&
      Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY) < 6
    )
      return;
    if (!drag.moved) board.setPointerCapture(event.pointerId);
    const origin = board.getBoundingClientRect();
    drag = {
      ...drag,
      moved: true,
      x: event.clientX - origin.left - drag.grabX,
      y: event.clientY - origin.top - drag.grabY,
    };
  }
  function up(event: PointerEvent) {
    if (!drag || event.pointerId !== drag.pointer) return;
    if (drag.moved && target) {
      moveTo(drag.id, target);
      suppressClick = true;
    }
    drag = null;
  }
  function click(event: MouseEvent) {
    if (suppressClick) {
      event.preventDefault();
      suppressClick = false;
    }
  }
  function keys(event: KeyboardEvent, id: string) {
    const step: Record<string, Cell> = {
      ArrowLeft: [-1, 0],
      ArrowRight: [1, 0],
      ArrowUp: [0, -1],
      ArrowDown: [0, 1],
    };
    if (!event.altKey || !step[event.key]) return;
    const from = placed.get(id);
    if (!from) return;
    event.preventDefault();
    const to: Cell = [
      Math.min(cols - 1, Math.max(0, from[0] + step[event.key][0])),
      Math.max(0, from[1] + step[event.key][1]),
    ];
    moveTo(id, to);
  }
  function host(url: string) {
    try {
      return new URL(url).hostname.replace(/^www\./, '');
    } catch {
      return url;
    }
  }
</script>

<div
  class="desktop-board"
  role="list"
  aria-label="桌面视图：按住拖动图标调整位置，或用 Alt + 方向键移动"
  bind:this={board}
  bind:clientWidth={width}
  on:pointermove={move}
  on:pointerup={up}
  on:pointercancel={() => (drag = null)}
  style:--rows={rows}
  style:--cell-w={CELL_W + 'px'}
  style:--cell-h={CELL_H + 'px'}
>
  {#if target}<div
      class="drop-cell"
      aria-hidden="true"
      style:transform={`translate(${target[0] * CELL_W}px, ${target[1] * CELL_H}px)`}
    ></div>{/if}
  {#each items as item (item.id)}
    {@const cell = placed.get(item.id) || [0, 0]}
    {@const dragging = drag?.moved && drag.id === item.id}
    <div
      class="desk-tile"
      class:dragging
      role="listitem"
      style:transform={dragging && drag
        ? `translate(${drag.x}px, ${drag.y}px)`
        : `translate(${cell[0] * CELL_W}px, ${cell[1] * CELL_H}px)`}
      on:pointerdown={(e) => down(e, item.id)}
      on:contextmenu|preventDefault={() => onEdit(item)}
    >
      <a
        href={item.url_raw}
        target={newTab ? '_blank' : '_self'}
        rel="noopener noreferrer"
        draggable="false"
        title={[item.title, item.url_raw, item.notes]
          .filter(Boolean)
          .join('\n')}
        on:click={click}
        on:keydown={(e) => keys(e, item.id)}
      >
        <span class="desk-icon" aria-hidden="true"
          >{#if item.icon}<img
              src={item.icon}
              alt=""
              draggable="false"
            />{:else}{(host(item.url_raw) || item.title)
              .slice(0, 1)
              .toUpperCase()}{/if}</span
        >
        <span class="desk-title">{item.title}</span>
      </a>
      <button
        class="desk-edit"
        title="编辑收藏"
        aria-label={'编辑 ' + item.title}
        on:pointerdown|stopPropagation
        on:click={() => onEdit(item)}><Icon name="edit" size={12} /></button
      >
    </div>
  {/each}
</div>

<style>
  .desktop-board {
    position: relative;
    min-height: calc(var(--rows) * var(--cell-h));
    margin: 4px 0 24px;
    user-select: none;
  }
  .desk-tile,
  .drop-cell {
    position: absolute;
    top: 0;
    left: 0;
    width: var(--cell-w);
    height: var(--cell-h);
  }
  .desk-tile {
    transition: transform 0.18s ease;
    border-radius: 10px;
  }
  .desk-tile.dragging {
    z-index: 3;
    transition: none;
    cursor: grabbing;
  }
  .desk-tile.dragging .desk-icon {
    box-shadow: 0 10px 26px #00000026;
    transform: scale(1.06);
  }
  .drop-cell {
    border: 1.5px dashed var(--accent);
    border-radius: 10px;
    background: color-mix(in srgb, var(--selected) 60%, transparent);
    pointer-events: none;
  }
  a {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    height: 100%;
    padding: 10px 6px 6px;
    border-radius: 10px;
    color: var(--text);
    text-align: center;
    cursor: default;
  }
  a:hover {
    background: var(--hover);
    color: var(--text);
  }
  .desk-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
    width: 52px;
    height: 52px;
    border: 1px solid var(--border);
    border-radius: 13px;
    background: var(--panel);
    color: var(--accent);
    font:
      500 22px Georgia,
      serif;
    overflow: hidden;
    transition:
      transform 0.15s,
      box-shadow 0.15s;
  }
  .desk-icon img {
    width: 30px;
    height: 30px;
    object-fit: contain;
  }
  .desk-title {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    width: 100%;
    font-size: 12px;
    line-height: 1.35;
    overflow-wrap: anywhere;
  }
  .desk-edit {
    position: absolute;
    top: 4px;
    right: 10px;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 50%;
    background: var(--panel);
    color: var(--muted);
    opacity: 0;
    transition: opacity 0.15s;
  }
  .desk-tile:hover .desk-edit,
  .desk-tile:focus-within .desk-edit {
    opacity: 1;
  }
  @media (hover: none) {
    .desk-edit {
      opacity: 1;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .desk-tile,
    .desk-icon {
      transition: none;
    }
  }
</style>
