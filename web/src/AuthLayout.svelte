<script lang="ts">
  export let setup = false;
</script>

<div class="archive-entry">
  <section class="entry-story" aria-labelledby="archive-heading">
    <a class="wordmark" href="/" aria-label="bookmarkd 首页">
      <svg
        width="22"
        height="29"
        viewBox="0 0 22 29"
        fill="none"
        aria-hidden="true"
      >
        <path d="M2 1.5h18v25L11 21l-9 5.5v-25Z" fill="currentColor" />
        <path d="M7 8h8M7 12h8" stroke="#f7f5ef" stroke-width="1.2" />
      </svg>
      <span>bookmarkd<span class="brand-dot">.</span></span>
    </a>

    <div class="story-copy">
      <p class="eyebrow">
        <span class="short-rule"></span> THE PERSONAL ARCHIVE
      </p>
      <h2 id="archive-heading">
        留住好奇，<br />日后相逢<span class="period">。</span>
      </h2>
      <p class="story-description">
        把偶然发现的美好，<br />收藏成自己的世界。
      </p>
    </div>

    <div class="archive-art" aria-hidden="true">
      <svg class="artwork" viewBox="0 0 640 380" fill="none">
        <defs>
          <linearGradient
            id="archive-paper"
            x1="271"
            y1="94"
            x2="384"
            y2="303"
            gradientUnits="userSpaceOnUse"
          >
            <stop stop-color="#fffdf7" />
            <stop offset="1" stop-color="#e7e3d6" />
          </linearGradient>
          <linearGradient
            id="archive-ribbon"
            x1="301"
            y1="82"
            x2="384"
            y2="295"
            gradientUnits="userSpaceOnUse"
          >
            <stop stop-color="#d97651" />
            <stop offset="1" stop-color="#b84e2e" />
          </linearGradient>
          <filter
            id="archive-shadow"
            x="-50%"
            y="-50%"
            width="200%"
            height="220%"
          >
            <feGaussianBlur stdDeviation="13" />
          </filter>
        </defs>

        <g class="orbital-lines" stroke="#a9aea0" stroke-width=".75">
          <ellipse
            cx="324"
            cy="243"
            rx="284"
            ry="90"
            transform="rotate(-13 324 243)"
          />
          <ellipse
            cx="324"
            cy="243"
            rx="235"
            ry="74"
            transform="rotate(-13 324 243)"
          />
          <ellipse
            cx="324"
            cy="243"
            rx="181"
            ry="57"
            transform="rotate(-13 324 243)"
          />
          <path d="M46 302 585 177" stroke-dasharray="2 6" opacity=".6" />
        </g>
        <ellipse
          cx="328"
          cy="291"
          rx="116"
          ry="18"
          fill="#777767"
          opacity=".15"
          filter="url(#archive-shadow)"
        />

        <g class="floating-archive">
          <g transform="translate(266 95) rotate(15)">
            <path
              d="M0 0h159v205H0z"
              fill="#d7d8c9"
              stroke="#afb2a1"
              stroke-width=".65"
            />
            <path
              d="M19 25h55M19 33h85M19 164h41M19 172h96"
              stroke="#929887"
              stroke-width="1"
            />
            <circle
              cx="79"
              cy="102"
              r="32"
              stroke="#9da38f"
              stroke-width=".7"
            />
            <circle
              cx="79"
              cy="102"
              r="22"
              stroke="#9da38f"
              stroke-width=".7"
            />
            <path d="M47 102h64M79 70v64" stroke="#9da38f" stroke-width=".7" />
          </g>

          <g transform="translate(218 73) rotate(-10)">
            <path
              d="M0 0h156l21 21v199H0z"
              fill="url(#archive-paper)"
              stroke="#bdbbab"
              stroke-width=".75"
            />
            <path
              d="M156 0v21h21"
              fill="#ddd9ca"
              stroke="#bdbbab"
              stroke-width=".75"
            />
            <path d="M20 25h43M20 31h27" stroke="#7c8273" stroke-width="1" />
            <path
              d="M20 166h82M20 174h121M20 182h112M20 190h53"
              stroke="#aaa99a"
              stroke-width="1"
            />
            <path d="M20 43h134" stroke="#d4d1c4" stroke-width=".7" />
            <circle cx="44" cy="211" r="1" fill="#a4a493" />
            <circle cx="50" cy="211" r="1" fill="#a4a493" />
            <circle cx="56" cy="211" r="1" fill="#a4a493" />
          </g>

          <g transform="translate(302 34) rotate(8)">
            <path
              d="M0 0h88v225l-44-33-44 33V0Z"
              fill="#914229"
              transform="translate(5 5)"
              opacity=".17"
            />
            <path d="M0 0h88v225l-44-33-44 33V0Z" fill="url(#archive-ribbon)" />
            <path
              d="M7 0v210l37-28 37 28V0"
              stroke="#efb398"
              stroke-width=".6"
              opacity=".65"
            />
            <path d="M24 37h40M24 43h25" stroke="#f7e1d3" stroke-width="1.2" />
            <path
              d="M44 79v56M16 107h56M24 87l40 40M24 127l40-40"
              stroke="#f8e6da"
              stroke-width="1.15"
            />
            <circle
              cx="44"
              cy="107"
              r="19"
              stroke="#f8e6da"
              stroke-width=".8"
            />
            <path d="M32 161h24" stroke="#f7e1d3" stroke-width=".8" />
          </g>
        </g>

        <circle cx="109" cy="280" r="5" fill="#c65b39" />
        <circle cx="548" cy="156" r="3" fill="#232822" />
        <g stroke="#747b68" stroke-width=".8">
          <path d="M128 103h15M135.5 95.5v15M497 294h11M502.5 288.5v11" />
        </g>
        <text
          x="82"
          y="340"
          fill="#626b5a"
          font-size="8"
          letter-spacing="1.5"
          font-family="monospace">COLLECT. CONNECT. REVISIT.</text
        >
        <path d="M396 334h164" stroke="#b8bcad" stroke-width=".7" />
        <text
          x="576"
          y="337"
          fill="#626b5a"
          font-size="8"
          font-family="monospace">↗</text
        >
      </svg>
    </div>

    <footer class="story-footer">
      <span>为每一次灵光，留一处归所。</span>
      <span class="tiny-spark" aria-hidden="true">✳</span>
    </footer>
  </section>

  <section
    class="entry-access"
    aria-label={setup ? '初始化私人档案馆' : '登录私人档案馆'}
  >
    <div class="access-topline">
      <span>PRIVATE BY DESIGN</span>
      <svg
        width="13"
        height="16"
        viewBox="0 0 13 16"
        fill="none"
        aria-hidden="true"
      >
        <path
          d="M3.5 6V4a3 3 0 0 1 6 0v2M1 6.5h11v8H1z"
          stroke="currentColor"
        />
        <path d="M6.5 9v3" stroke="currentColor" />
      </svg>
    </div>
    <div class="access-body">
      <p class="access-number">
        <span>{setup ? '01' : '↗'}</span> / {setup
          ? 'A NEW BEGINNING'
          : 'YOUR COLLECTION AWAITS'}
      </p>
      <div class="auth-content"><slot /></div>
      <div class="access-note">
        <span class="note-line" aria-hidden="true"></span>
        <p>使用通行密钥安全验证，无需记住密码。</p>
      </div>
    </div>
    <footer class="access-footer">
      <span><span class="status-dot"></span> LOCAL-FIRST</span>
      <span>YOURS, ALWAYS.</span>
    </footer>
  </section>
</div>

<style>
  .archive-entry {
    --entry-paper: #f7f5ef;
    --entry-ink: #232822;
    --entry-muted: #626a5d;
    --entry-accent: #c65b39;
    --entry-serif:
      'Iowan Old Style', 'Baskerville', 'Songti SC', 'STSong',
      'Noto Serif CJK SC', serif;
    min-height: 100svh;
    display: grid;
    grid-template-columns: 1.12fr 1fr;
    color: var(--entry-ink);
    background: var(--entry-paper);
    color-scheme: light;
  }
  .entry-story {
    position: relative;
    min-height: 760px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    background: #edeedf;
    padding: 40px clamp(32px, 4.9vw, 80px) 30px;
    border-right: 1px solid #d7d9cb;
  }
  .wordmark {
    display: inline-flex;
    align-items: center;
    align-self: flex-start;
    gap: 13px;
    color: var(--entry-ink);
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    font-size: 31px;
    font-weight: 650;
    letter-spacing: -1.6px;
    line-height: 1.15;
    text-decoration: none;
  }
  .wordmark svg {
    color: var(--entry-accent);
  }
  .brand-dot {
    color: var(--entry-accent);
  }
  .story-copy {
    position: relative;
    z-index: 1;
    margin-top: clamp(55px, 7.5vh, 105px);
  }
  .eyebrow,
  .access-topline,
  .access-number,
  .access-footer {
    font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', monospace;
    font-weight: 400;
    font-size: 10px;
    letter-spacing: 1.7px;
    line-height: 1.6;
  }
  .eyebrow {
    display: flex;
    align-items: center;
    gap: 11px;
    margin: 0 0 22px;
    color: #626b5a;
  }
  .short-rule {
    width: 24px;
    height: 1px;
    background: var(--entry-accent);
  }
  .story-copy h2 {
    margin: 0;
    color: var(--entry-ink);
    font-family: var(--entry-serif);
    font-size: clamp(46px, 4.7vw, 78px);
    font-weight: 400;
    line-height: 1.28;
    letter-spacing: -3px;
  }
  .period {
    color: var(--entry-accent);
  }
  .story-description {
    margin: 23px 0 0;
    color: #626b5a;
    font-size: 13px;
    line-height: 1.95;
    letter-spacing: 1px;
  }
  .archive-art {
    position: relative;
    margin: -14px -65px -7px;
    flex: 1;
    display: grid;
    align-items: end;
    min-height: 270px;
  }
  .artwork {
    width: 100%;
    max-width: 680px;
    height: auto;
    margin: 0 auto;
    overflow: visible;
  }
  .floating-archive {
    animation: archive-float 8s ease-in-out infinite;
  }
  .story-footer {
    position: relative;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding-top: 14px;
    border-top: 1px solid #cfd3c2;
    font-size: 10px;
    letter-spacing: 1px;
    color: #626b5a;
  }
  .tiny-spark {
    color: var(--entry-accent);
    font-size: 20px;
    line-height: 1;
  }
  .entry-access {
    min-width: 0;
    padding: 44px clamp(35px, 5.2vw, 90px) 30px;
    display: flex;
    flex-direction: column;
  }
  .access-topline {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 15px;
    color: var(--entry-muted);
  }
  .access-body {
    width: 100%;
    max-width: 378px;
    margin: auto;
    padding: 70px 0;
    animation: archive-arrive 0.8s cubic-bezier(0.2, 0.7, 0.2, 1) both;
  }
  .access-number {
    margin: 0 0 33px;
    color: var(--entry-muted);
    font-size: 9px;
    letter-spacing: 1.1px;
  }
  .access-number > span {
    display: inline-block;
    color: #b24d2e;
    margin-right: 11px;
    font-size: 17px;
  }
  .auth-content :global(.auth) {
    width: 100%;
    margin: 0;
    padding: 0;
    text-align: left;
    background: transparent;
    border: 0;
    border-radius: 0;
    box-shadow: none;
  }
  .auth-content :global(.auth h1) {
    margin: 0 0 16px;
    font-family: var(--entry-serif);
    color: var(--entry-ink);
    font-size: clamp(31px, 2.8vw, 44px);
    font-weight: 400;
    letter-spacing: -1px;
    line-height: 1.45;
  }
  .auth-content :global(.auth .muted) {
    color: var(--entry-muted);
    font-size: 13px;
    line-height: 1.8;
    margin-bottom: 29px;
  }
  .auth-content :global(.auth .auth-description) {
    margin: 0;
    color: var(--entry-muted);
    font-size: 13px;
    line-height: 1.85;
  }
  .auth-content :global(.auth .primary) {
    width: 100%;
    min-height: 54px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    margin-top: 35px;
    padding: 14px 18px;
    background: var(--entry-ink);
    color: var(--entry-paper);
    border: 1px solid var(--entry-ink);
    border-radius: 3px;
    font-size: 13px;
    letter-spacing: 0.5px;
    box-shadow: none;
    transition:
      background 0.2s,
      transform 0.2s,
      box-shadow 0.2s;
  }
  .auth-content :global(.auth .primary:hover:not(:disabled)) {
    transform: translateY(-2px);
    background: #3e463a;
    border-color: #3e463a;
    box-shadow: 0 5px 12px #23282214;
  }
  .auth-content :global(.auth .primary:active:not(:disabled)) {
    transform: translateY(0);
  }
  .auth-content :global(.auth .primary:focus-visible) {
    outline: 2px solid var(--entry-accent);
    outline-offset: 4px;
  }
  .auth-content :global(.auth label) {
    color: var(--entry-muted);
    font-size: 11px;
  }
  .auth-content :global(.auth .remember) {
    margin: 17px 0 0;
    display: flex;
    align-items: center;
    gap: 9px;
    justify-content: flex-start;
  }
  .auth-content :global(.auth input[type='checkbox']) {
    width: 13px;
    height: 13px;
    margin: 0;
    accent-color: var(--entry-accent);
  }
  .auth-content :global(.auth input:not([type='checkbox'])) {
    background: #fffdf7;
    color: var(--entry-ink);
    border: 1px solid #cbd0c2;
    border-radius: 3px;
    padding: 13px;
    margin-top: 8px;
  }
  .auth-content :global(.auth input:focus-visible) {
    outline: 2px solid var(--entry-accent);
    outline-offset: 2px;
  }
  .auth-content :global(.auth .error) {
    color: #a63f29;
    margin: 16px 0 0;
    font-size: 12px;
  }
  .auth-content :global(.auth .error:empty) {
    display: none;
  }
  .access-note {
    display: flex;
    align-items: flex-start;
    gap: 13px;
    margin-top: 45px;
  }
  .note-line {
    width: 18px;
    height: 1px;
    flex-shrink: 0;
    background: #b3b9a9;
    margin-top: 9px;
  }
  .access-note p {
    margin: 0;
    font-size: 11px;
    color: var(--entry-muted);
    line-height: 1.9;
  }
  .access-footer {
    display: flex;
    justify-content: space-between;
    gap: 15px;
    padding-top: 14px;
    border-top: 1px solid #dddcd3;
    color: var(--entry-muted);
    font-size: 8px;
    letter-spacing: 1.3px;
  }
  .access-footer > span:first-child {
    display: inline-flex;
    align-items: center;
    gap: 7px;
  }
  .status-dot {
    width: 4px;
    height: 4px;
    background: #6c805d;
    border-radius: 50%;
  }
  @keyframes archive-float {
    0%,
    100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-9px);
    }
  }
  @keyframes archive-arrive {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  @media (min-width: 1550px) {
    .entry-story {
      padding-left: 7vw;
      padding-right: 7vw;
    }
    .archive-art {
      margin-top: 5px;
    }
  }
  @media (max-width: 1000px) {
    .archive-entry {
      grid-template-columns: 1fr 1fr;
    }
    .entry-story {
      padding: 33px 32px 25px;
    }
    .entry-access {
      padding: 36px 32px 25px;
    }
    .wordmark {
      font-size: 28px;
    }
    .story-copy h2 {
      font-size: 49px;
      letter-spacing: -2px;
    }
    .archive-art {
      margin-right: -55px;
      margin-left: -55px;
    }
    .access-footer {
      font-size: 7px;
      letter-spacing: 0.8px;
    }
  }
  @media (max-width: 700px) {
    .archive-entry {
      grid-template-columns: 1fr;
      min-height: 100svh;
    }
    .entry-story {
      min-height: 0;
      padding: 26px 27px 22px;
      border-right: 0;
      border-bottom: 1px solid #d7d9cb;
    }
    .wordmark {
      font-size: 26px;
      gap: 10px;
    }
    .wordmark svg {
      width: 18px;
      height: 24px;
    }
    .story-copy {
      margin-top: 39px;
      margin-bottom: 25px;
    }
    .eyebrow {
      font-size: 8px;
      letter-spacing: 1.3px;
      margin-bottom: 15px;
    }
    .story-copy h2 {
      font-size: clamp(38px, 8.5vw, 54px);
      line-height: 1.24;
      letter-spacing: -2px;
    }
    .story-description {
      margin-top: 16px;
      font-size: 11px;
      line-height: 1.8;
    }
    .archive-art {
      position: absolute;
      width: 340px;
      right: -135px;
      bottom: 38px;
      min-height: 0;
      margin: 0;
      opacity: 0.86;
      pointer-events: none;
    }
    .artwork {
      transform: rotate(-14deg);
    }
    .story-footer {
      font-size: 9px;
      padding-top: 12px;
    }
    .tiny-spark {
      font-size: 16px;
    }
    .entry-access {
      padding: 25px 27px 24px;
      min-height: 395px;
    }
    .access-topline {
      font-size: 8px;
      letter-spacing: 1.2px;
    }
    .access-topline svg {
      width: 10px;
      height: 13px;
    }
    .access-body {
      max-width: 420px;
      padding: 31px 0 43px;
    }
    .access-number {
      margin-bottom: 15px;
      font-size: 8px;
    }
    .auth-content :global(.auth h1) {
      font-size: 30px;
      margin-bottom: 10px;
    }
    .auth-content :global(.auth .primary) {
      min-height: 50px;
      margin-top: 25px;
    }
    .access-note {
      margin-top: 28px;
    }
    .access-footer {
      font-size: 8px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .floating-archive,
    .access-body {
      animation: none;
    }
    .auth-content :global(.auth .primary) {
      transition: none;
    }
    .auth-content :global(.auth .primary:hover:not(:disabled)) {
      transform: none;
    }
  }
</style>
