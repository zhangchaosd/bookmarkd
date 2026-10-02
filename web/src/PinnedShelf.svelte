<script lang="ts">
  import Icon from './Icon.svelte';
  export let items: {
    id: string;
    title: string;
    url_raw: string;
    notes: string;
    tags: string[];
  }[] = [];
  export let newTab = true;
  function domain(url: string) {
    try {
      return new URL(url).hostname.replace(/^www\./, '');
    } catch {
      return url;
    }
  }
</script>

<section class="pinned-shelf" aria-label="精选置顶">
  <div class="shelf-heading">
    <span><Icon name="pin" size={13} /> 常看，常新</span><span
      class="shelf-caption"
      >THE ESSENTIALS / {String(items.length).padStart(2, '0')}</span
    >
  </div>
  <div class="shelf-grid">
    {#each items as item, i (item.id)}
      <a
        class="shelf-card"
        href={item.url_raw}
        target={newTab ? '_blank' : '_self'}
        rel="noopener noreferrer"
        aria-label={'打开置顶收藏：' + item.title}
      >
        <div class="card-top">
          <span class="card-domain">{domain(item.url_raw)}</span><span
            class="card-arrow">↗</span
          >
        </div>
        <h2>{item.title}</h2>
        <div class="card-bottom">
          <span>{item.tags[0] || '值得再次打开'}</span><span class="card-index"
            >0{i + 1}</span
          >
        </div>
        <div class="card-art" aria-hidden="true">
          <i></i><i></i><i></i><i></i>
        </div>
      </a>
    {/each}
  </div>
</section>

<style>
  .pinned-shelf {
    margin: 0 0 30px;
  }
  .shelf-heading {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 13px;
    font-size: 12px;
    color: var(--muted);
  }
  .shelf-heading > span:first-child {
    display: flex;
    gap: 8px;
    align-items: center;
    color: var(--text);
  }
  .shelf-caption {
    font: 9px var(--mono, monospace);
    letter-spacing: 1.3px;
  }
  .shelf-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 14px;
  }
  .shelf-card {
    position: relative;
    isolation: isolate;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    min-height: 158px;
    padding: 20px 22px 16px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--panel);
    transition:
      transform 0.25s,
      box-shadow 0.25s,
      border-color 0.25s;
  }
  .shelf-card:first-child {
    background: #2d382f;
    color: #f4f2e9;
    border-color: #2d382f;
  }
  .shelf-card:nth-child(2) {
    background: color-mix(in srgb, var(--selected) 60%, var(--panel));
  }
  .shelf-card:hover {
    transform: translateY(-4px);
    box-shadow: 0 10px 24px #2528220b;
    border-color: var(--accent);
  }
  .shelf-card:first-child:hover {
    color: #fff;
  }
  .card-top,
  .card-bottom {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    position: relative;
    z-index: 1;
  }
  .card-domain {
    font: 10px var(--mono, monospace);
    opacity: 0.75;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .card-arrow {
    font-size: 20px;
    line-height: 1;
    transition: transform 0.2s;
  }
  .shelf-card:hover .card-arrow {
    transform: translate(2px, -2px);
  }
  h2 {
    position: relative;
    z-index: 1;
    font-size: 18px;
    font-weight: 500;
    line-height: 1.5;
    margin: 19px 25px 20px 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    letter-spacing: -0.4px;
  }
  .card-bottom {
    margin-top: auto;
    font-size: 10px;
  }
  .card-bottom > span:first-child {
    opacity: 0.75;
  }
  .card-index {
    font:
      italic 18px Georgia,
      serif;
    opacity: 0.7;
  }
  .card-art {
    z-index: 0;
    position: absolute;
    right: 15px;
    bottom: -32px;
    width: 100px;
    height: 120px;
    transform: rotate(30deg);
    opacity: 0.1;
    pointer-events: none;
  }
  .card-art i {
    position: absolute;
    inset: 0;
    border: 1px solid currentColor;
    border-radius: 60px 60px 0 0;
  }
  .card-art i:nth-child(2) {
    inset: 10px;
  }
  .card-art i:nth-child(3) {
    inset: 20px;
  }
  .card-art i:nth-child(4) {
    inset: 30px;
  }
  .shelf-card:first-child .card-art {
    opacity: 0.22;
  }
  @media (max-width: 1100px) {
    .shelf-card {
      padding: 16px;
    }
    h2 {
      font-size: 16px;
    }
  }
  @media (max-width: 700px) {
    .pinned-shelf {
      margin-bottom: 24px;
    }
    .shelf-grid {
      grid-template-columns: repeat(3, minmax(230px, 1fr));
      overflow-x: auto;
      padding: 4px 0 8px;
      scroll-snap-type: x mandatory;
    }
    .shelf-card {
      scroll-snap-align: start;
      min-height: 148px;
    }
    .shelf-caption {
      font-size: 8px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .shelf-card,
    .card-arrow {
      transition: none;
    }
    .shelf-card:hover {
      transform: none;
    }
  }
</style>
