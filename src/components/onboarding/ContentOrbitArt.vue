<template>
  <div class="content-art" aria-hidden="true">
    <svg viewBox="0 0 820 620">
      <defs>
        <linearGradient id="contentBackdrop" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#fffaf5" />
          <stop offset=".6" stop-color="#f0e4ee" />
          <stop offset="1" stop-color="#e7dded" />
        </linearGradient>
        <radialGradient id="contentCoreGlow" cx="50%" cy="50%" r="50%">
          <stop offset="0" stop-color="#fff7c9" stop-opacity=".95" />
          <stop offset=".45" stop-color="#f5c95b" stop-opacity=".32" />
          <stop offset="1" stop-color="#f5c95b" stop-opacity="0" />
        </radialGradient>
        <linearGradient id="contentPaper" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#fff" />
          <stop offset="1" stop-color="#f2edf7" />
        </linearGradient>
        <linearGradient id="contentPurple" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#9b62bb" />
          <stop offset="1" stop-color="#5e397e" />
        </linearGradient>
        <linearGradient id="contentDisc" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#8fd0df" />
          <stop offset=".28" stop-color="#efe8ff" />
          <stop offset=".52" stop-color="#f4c869" />
          <stop offset=".76" stop-color="#cd7cb3" />
          <stop offset="1" stop-color="#739ec9" />
        </linearGradient>
        <pattern id="contentDots" width="20" height="20" patternUnits="userSpaceOnUse">
          <circle cx="2" cy="2" r="1" fill="#71498e" fill-opacity=".1" />
        </pattern>
        <filter id="contentShadow" x="-40%" y="-40%" width="180%" height="190%">
          <feDropShadow dx="0" dy="20" stdDeviation="22" flood-color="#4f315f" flood-opacity=".16" />
        </filter>
        <filter id="contentSmallShadow" x="-50%" y="-50%" width="200%" height="210%">
          <feDropShadow dx="0" dy="10" stdDeviation="12" flood-color="#4f315f" flood-opacity=".17" />
        </filter>
        <clipPath id="contentClip"><rect x="24" y="24" width="772" height="572" rx="62" /></clipPath>
      </defs>

      <g clip-path="url(#contentClip)">
        <rect x="24" y="24" width="772" height="572" rx="62" fill="url(#contentBackdrop)" />
        <rect x="24" y="24" width="772" height="572" fill="url(#contentDots)" />
        <circle cx="413" cy="308" r="176" fill="url(#contentCoreGlow)" class="core-glow" />

        <g class="orbit-lines" fill="none" stroke="#7b4aa0" stroke-opacity=".2">
          <ellipse cx="410" cy="309" rx="304" ry="190" stroke-width="1.5" stroke-dasharray="5 13" />
          <ellipse cx="410" cy="309" rx="240" ry="251" stroke-width="1" stroke-dasharray="2 11" transform="rotate(20 410 309)" />
          <path d="M159 222 C250 120 409 99 550 154 C647 191 700 274 690 355" stroke-width="3" stroke-linecap="round" />
          <path d="M157 389 C232 506 377 546 520 507 C606 483 670 428 690 361" stroke-width="3" stroke-linecap="round" />
        </g>

        <g class="central-material" filter="url(#contentShadow)">
          <path d="M301 153 L522 171 L507 481 L287 462 Z" fill="#d8cbe0" opacity=".45" transform="rotate(-4 404 317)" />
          <rect x="298" y="147" width="224" height="322" rx="28" fill="url(#contentPaper)" />
          <path d="M298 203 H522" stroke="#7b4aa0" stroke-opacity=".11" stroke-width="2" />
          <rect x="326" y="177" width="58" height="10" rx="5" fill="#7b4aa0" opacity=".23" />
          <circle cx="484" cy="182" r="9" fill="#f5c95b" />

          <g fill="#5d4770" opacity=".22">
            <rect x="330" y="238" width="162" height="11" rx="5.5" />
            <rect x="330" y="264" width="124" height="11" rx="5.5" />
            <rect x="330" y="290" width="148" height="11" rx="5.5" />
            <rect x="330" y="347" width="150" height="11" rx="5.5" />
            <rect x="330" y="373" width="114" height="11" rx="5.5" />
            <rect x="330" y="399" width="142" height="11" rx="5.5" />
          </g>
          <g fill="#7b4aa0">
            <circle cx="316" cy="243" r="4" />
            <circle cx="316" cy="269" r="4" opacity=".72" />
            <circle cx="316" cy="295" r="4" opacity=".5" />
          </g>
          <path d="M298 321 H522" stroke="#f1bd4b" stroke-width="8" stroke-linecap="round" opacity=".68" />
          <path d="M353 321 H471" stroke="#fff7c9" stroke-width="3" stroke-linecap="round" />

          <g transform="translate(438 116) rotate(8)">
            <g class="material-clip">
            <path d="M0 53 V20 C0 -8 43 -9 43 18 V70 C43 95 9 101 -2 82 C-9 69 -3 54 10 49 L29 41" fill="none" stroke="#6e468a" stroke-width="10" stroke-linecap="round" />
            <path d="M13 22 V62 C13 72 25 77 31 69" fill="none" stroke="#f2bf52" stroke-width="6" stroke-linecap="round" />
            </g>
          </g>
        </g>

        <g transform="translate(135 348)" filter="url(#contentSmallShadow)">
          <g class="disc media-object">
          <circle r="90" fill="#5f4373" opacity=".12" />
          <circle r="78" fill="url(#contentDisc)" />
          <circle r="67" fill="none" stroke="#fff" stroke-opacity=".45" stroke-width="2" />
          <circle r="54" fill="none" stroke="#5e397e" stroke-opacity=".24" stroke-width="1.5" />
          <circle r="41" fill="none" stroke="#fff" stroke-opacity=".38" stroke-width="1" />
          <circle r="18" fill="#f6efe9" />
          <circle r="7" fill="#6d4787" />
          <path d="M-51 -43 L-13 -14 M18 18 L55 48" stroke="#fff" stroke-opacity=".7" stroke-width="7" stroke-linecap="round" />
          </g>
        </g>

        <g transform="translate(191 162) rotate(-8)" filter="url(#contentSmallShadow)">
          <g class="film media-object">
          <circle r="79" fill="#273040" />
          <circle r="64" fill="#3f4653" stroke="#e8d6a4" stroke-opacity=".28" stroke-width="2" />
          <circle r="14" fill="#d1ae63" />
          <g fill="#ede4d5">
            <circle cx="0" cy="-37" r="16" />
            <circle cx="35" cy="-11" r="16" />
            <circle cx="22" cy="31" r="16" />
            <circle cx="-22" cy="31" r="16" />
            <circle cx="-35" cy="-11" r="16" />
          </g>
          <path d="M59 42 C95 59 104 91 94 123 C86 148 99 166 128 174" fill="none" stroke="#273040" stroke-width="18" stroke-linecap="round" />
          <path d="M61 42 C94 60 98 92 89 120 C82 143 96 160 126 168" fill="none" stroke="#e8d6a4" stroke-opacity=".5" stroke-width="2" stroke-dasharray="5 8" />
          </g>
        </g>

        <g transform="translate(649 174)" filter="url(#contentSmallShadow)">
          <g class="bird media-object">
          <path d="M-83 9 C-52 12 -27 -2 -9 -25 C-7 -43 4 -57 21 -63 C37 -69 55 -64 65 -53 C82 -56 96 -63 108 -76 C105 -59 97 -47 84 -38 C98 -38 110 -42 121 -49 C115 -35 104 -25 88 -19 C83 22 61 53 26 68 C-8 83 -46 75 -73 50 C-51 51 -33 43 -19 30 C-45 28 -67 21 -83 9 Z" fill="#4c9bd0" />
          <path d="M-22 30 C3 25 25 8 40 -17 C35 9 21 32 -1 48" fill="#77b9df" opacity=".72" />
          <circle cx="44" cy="-42" r="4" fill="#f8f4ee" />
          </g>
        </g>

        <g transform="translate(645 425)" filter="url(#contentSmallShadow)">
          <g class="figure media-object">
          <ellipse cx="0" cy="96" rx="81" ry="21" fill="#5e397e" opacity=".16" />
          <ellipse cx="0" cy="88" rx="54" ry="13" fill="#6e468a" />
          <rect x="-36" y="64" width="72" height="25" rx="8" fill="#8e65a7" />
          <circle cx="0" cy="-61" r="24" fill="#2e2938" />
          <path d="M-23 -55 C-41 -36 -44 8 -34 42 L-12 30 L0 67 L13 30 L35 42 C44 5 39 -35 22 -55 C13 -42 -14 -42 -23 -55 Z" fill="#35303f" />
          <path d="M-27 -44 C-18 -7 -23 22 -42 46 C-22 37 -8 24 -2 7 C5 27 18 41 40 48 C22 19 19 -14 28 -44 Z" fill="#5d3b73" />
          <path d="M-10 -31 C-6 -27 6 -27 10 -31 L8 -12 C4 -7 -4 -7 -8 -12 Z" fill="#f0c1a4" />
          <path d="M-17 7 L-45 34 M17 7 L43 31 M-10 58 L-22 78 M10 58 L22 78" fill="none" stroke="#292532" stroke-width="8" stroke-linecap="round" />
          <path d="M-23 -57 C-25 -82 20 -83 25 -57 C11 -67 -7 -67 -23 -57 Z" fill="#27232e" />
          </g>
        </g>

        <g class="conversion-rays" fill="none" stroke="#f0bd50" stroke-linecap="round">
          <path d="M280 313 H241" stroke-width="4" />
          <path d="M540 313 H579" stroke-width="4" />
          <path d="M410 126 V90" stroke-width="4" />
          <path d="M410 490 V527" stroke-width="4" />
        </g>

        <g class="small-sparks" fill="#f0bd50">
          <path d="M266 115 l5 12 12 5-12 5-5 12-5-12-12-5 12-5z" />
          <path d="M588 503 l4 10 10 4-10 4-4 10-4-10-10-4 10-4z" />
          <circle cx="690" cy="308" r="6" fill="#7b4aa0" />
          <circle cx="122" cy="266" r="5" fill="#dc8b78" />
        </g>
      </g>

      <rect x="24" y="24" width="772" height="572" rx="62" fill="none" stroke="#7b4aa0" stroke-opacity=".14" stroke-width="1.5" />
    </svg>
  </div>
</template>

<style scoped>
.content-art { width: min(100%, 820px); height: 100%; }
svg { width: 100%; height: 100%; overflow: visible; }
.core-glow { animation: core-breathe 4.8s ease-in-out infinite; transform-origin: 413px 308px; }
.orbit-lines ellipse:first-child { animation: orbit-clockwise 42s linear infinite; transform-origin: 410px 309px; }
.orbit-lines ellipse:nth-child(2) { animation: orbit-counter 36s linear infinite; transform-origin: 410px 309px; }
.central-material { animation: material-float 5.4s ease-in-out infinite; transform-origin: 410px 320px; }
.material-clip { animation: clip-wiggle 4s ease-in-out infinite; transform-box: fill-box; transform-origin: center; }
.disc { animation: disc-float 5.6s ease-in-out infinite; transform-box: fill-box; transform-origin: center; }
.disc circle:not(:first-child) { transform-origin: center; animation: disc-spin 18s linear infinite; }
.film { animation: film-float 5s ease-in-out infinite; transform-box: fill-box; transform-origin: center; }
.bird { animation: bird-float 4.4s ease-in-out infinite; transform-box: fill-box; transform-origin: center; }
.figure { animation: figure-float 5.2s ease-in-out infinite; transform-box: fill-box; transform-origin: bottom; }
.conversion-rays { animation: ray-pulse 2.8s ease-in-out infinite; }
.small-sparks { animation: spark-pulse 3.4s ease-in-out infinite; transform-origin: center; }
@keyframes core-breathe { 0%, 100% { opacity: .7; transform: scale(.92); } 50% { opacity: 1; transform: scale(1.08); } }
@keyframes orbit-clockwise { to { transform: rotate(360deg); } }
@keyframes orbit-counter { to { transform: rotate(-360deg); } }
@keyframes material-float { 0%, 100% { transform: translateY(0) rotate(-.25deg); } 50% { transform: translateY(-9px) rotate(.3deg); } }
@keyframes clip-wiggle { 0%, 100% { transform: rotate(0); } 50% { transform: rotate(5deg) translateY(-4px); } }
@keyframes disc-float { 0%, 100% { transform: translateY(0) rotate(-1deg); } 50% { transform: translateY(-10px) rotate(2deg); } }
@keyframes disc-spin { to { transform: rotate(360deg); } }
@keyframes film-float { 0%, 100% { transform: translateY(0) rotate(-2deg); } 50% { transform: translateY(-8px) rotate(2deg); } }
@keyframes bird-float { 0%, 100% { transform: translateY(0) rotate(-1deg); } 50% { transform: translateY(-12px) rotate(2deg); } }
@keyframes figure-float { 0%, 100% { transform: translateY(0); } 50% { transform: translateY(-7px); } }
@keyframes ray-pulse { 0%, 100% { opacity: .4; } 50% { opacity: 1; } }
@keyframes spark-pulse { 0%, 100% { opacity: .55; transform: scale(.94); } 50% { opacity: 1; transform: scale(1.08); } }
</style>
