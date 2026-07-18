<template>
  <div class="system-art" aria-hidden="true">
    <svg viewBox="0 0 820 620">
      <defs>
        <linearGradient id="systemBackdrop" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#f8fbf7" />
          <stop offset=".56" stop-color="#e4efeb" />
          <stop offset="1" stop-color="#d9e8e3" />
        </linearGradient>
        <radialGradient id="systemHalo" cx="50%" cy="50%" r="50%">
          <stop offset="0" stop-color="#a9ddd2" stop-opacity=".65" />
          <stop offset="1" stop-color="#a9ddd2" stop-opacity="0" />
        </radialGradient>
        <linearGradient id="systemCore" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#386f6b" />
          <stop offset="1" stop-color="#183f45" />
        </linearGradient>
        <linearGradient id="systemGold" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#f4dc84" />
          <stop offset="1" stop-color="#d3a548" />
        </linearGradient>
        <pattern id="systemGrid" width="31" height="31" patternUnits="userSpaceOnUse">
          <path d="M31 0 H0 V31" fill="none" stroke="#286f68" stroke-opacity=".055" stroke-width="1" />
        </pattern>
        <filter id="systemShadow" x="-50%" y="-50%" width="200%" height="210%">
          <feDropShadow dx="0" dy="18" stdDeviation="20" flood-color="#1c4d49" flood-opacity=".15" />
        </filter>
        <filter id="systemGlow" x="-80%" y="-80%" width="260%" height="260%">
          <feGaussianBlur stdDeviation="5" result="b" />
          <feMerge><feMergeNode in="b" /><feMergeNode in="SourceGraphic" /></feMerge>
        </filter>
        <clipPath id="systemClip"><rect x="24" y="24" width="772" height="572" rx="62" /></clipPath>
      </defs>

      <g clip-path="url(#systemClip)">
        <rect x="24" y="24" width="772" height="572" rx="62" fill="url(#systemBackdrop)" />
        <rect x="24" y="24" width="772" height="572" fill="url(#systemGrid)" />
        <circle cx="394" cy="292" r="224" fill="url(#systemHalo)" class="system-halo" />

        <g class="level-rail" transform="translate(92 125)">
          <path d="M24 353 C46 307 65 261 78 212 C91 164 105 119 139 68" fill="none" stroke="#286f68" stroke-opacity=".2" stroke-width="3" stroke-dasharray="4 10" />
          <g transform="translate(24 353)"><g class="level-node"><circle r="23" fill="#f7faf7" stroke="#286f68" stroke-opacity=".18" /><circle r="7" fill="#a7c8c0" /></g></g>
          <g transform="translate(47 301)"><g class="level-node"><circle r="25" fill="#f7faf7" stroke="#286f68" stroke-opacity=".2" /><circle r="8" fill="#82b5aa" /></g></g>
          <g transform="translate(72 238)"><g class="level-node"><circle r="28" fill="#f7faf7" stroke="#286f68" stroke-opacity=".24" /><circle r="9" fill="#5c9a90" /></g></g>
          <g transform="translate(101 162)"><g class="level-node"><circle r="31" fill="#f7faf7" stroke="#286f68" stroke-opacity=".28" /><circle r="10" fill="#397d74" /></g></g>
          <g transform="translate(139 68)"><g class="level-node"><circle r="35" fill="#f7faf7" stroke="#286f68" stroke-opacity=".34" /><circle r="11" fill="#1e625d" /></g></g>
        </g>

        <g class="grammar-tree" fill="none" stroke="#2d716b" stroke-opacity=".23" stroke-width="2">
          <path d="M204 202 C235 202 235 171 269 171 H298" />
          <path d="M204 202 C235 202 235 232 269 232 H298" />
          <path d="M204 202 C234 202 236 278 276 278 H303" />
          <path d="M203 180 V224 M194 180 H211 M194 224 H211" />
        </g>

        <g class="core-cards" filter="url(#systemShadow)">
          <rect x="282" y="135" width="226" height="298" rx="42" fill="#f8fbf8" stroke="#286f68" stroke-opacity=".12" stroke-width="2" />
          <path d="M282 237 H508" stroke="#286f68" stroke-opacity=".1" stroke-width="2" />
          <path d="M394 135 V433" stroke="#286f68" stroke-opacity=".08" stroke-width="2" />
          <circle cx="394" cy="283" r="96" fill="none" stroke="#286f68" stroke-opacity=".11" stroke-width="1.5" stroke-dasharray="4 9" />
          <circle cx="394" cy="283" r="69" fill="url(#systemCore)" />
          <circle cx="394" cy="283" r="55" fill="none" stroke="#e8f8f4" stroke-opacity=".22" stroke-width="1.5" />
          <text x="369" y="298" fill="#f5fbf9" font-family="Yu Gothic, Meiryo, sans-serif" font-size="48" font-weight="600">あ</text>
          <text x="402" y="315" fill="#f2cf6d" font-family="Yu Gothic, Meiryo, sans-serif" font-size="36" font-weight="600">ア</text>

          <g fill="#286f68" opacity=".2">
            <rect x="319" y="380" width="150" height="8" rx="4" />
            <rect x="338" y="401" width="112" height="8" rx="4" />
            <rect x="318" y="173" width="67" height="8" rx="4" />
          </g>
          <g transform="translate(452 171)">
            <circle r="20" fill="#f4d676" />
            <path d="M-9 0 H9 M0 -9 V9" stroke="#285e5a" stroke-width="3.5" stroke-linecap="round" />
          </g>
        </g>

        <g class="knowledge-network">
          <g fill="none" stroke="#286f68" stroke-opacity=".25" stroke-width="2">
            <path d="M493 190 C547 139 612 131 672 164" />
            <path d="M498 237 C561 218 622 221 696 262" />
            <path d="M504 293 C573 293 620 323 682 365" />
            <path d="M485 344 C547 379 592 418 641 461" />
            <path d="M569 157 L600 228 L664 166" />
            <path d="M600 228 L657 303 L696 262" />
            <path d="M657 303 L620 386 L682 365" />
            <path d="M620 386 L641 461" />
          </g>
          <g fill="#f7faf7" stroke="#286f68" stroke-width="3">
            <circle cx="569" cy="157" r="14" />
            <circle cx="664" cy="166" r="19" />
            <circle cx="600" cy="228" r="23" />
            <circle cx="696" cy="262" r="13" />
            <circle cx="657" cy="303" r="18" />
            <circle cx="620" cy="386" r="25" />
            <circle cx="682" cy="365" r="12" />
            <circle cx="641" cy="461" r="18" />
          </g>
          <g fill="#286f68">
            <circle cx="569" cy="157" r="5" />
            <circle cx="664" cy="166" r="7" />
            <circle cx="600" cy="228" r="8" />
            <circle cx="696" cy="262" r="4" />
            <circle cx="657" cy="303" r="6" />
            <circle cx="620" cy="386" r="9" />
            <circle cx="682" cy="365" r="4" />
            <circle cx="641" cy="461" r="6" />
          </g>
          <circle cx="600" cy="228" r="34" fill="none" stroke="#e0bc5a" stroke-width="2" stroke-dasharray="3 7" class="network-orbit" />
          <circle cx="620" cy="386" r="37" fill="none" stroke="#e0bc5a" stroke-width="2" stroke-dasharray="3 8" class="network-orbit network-orbit--slow" />
        </g>

        <g transform="translate(153 349) rotate(-7)" filter="url(#systemShadow)">
          <g class="card-stack">
          <rect x="18" y="12" width="115" height="145" rx="19" fill="#bdd7d0" />
          <rect x="9" y="6" width="115" height="145" rx="19" fill="#dce9e4" />
          <rect width="115" height="145" rx="19" fill="#fbfdfb" stroke="#286f68" stroke-opacity=".15" />
          <circle cx="57" cy="56" r="26" fill="#e8f1ed" />
          <path d="M44 56 H70 M57 43 V69" stroke="#286f68" stroke-width="5" stroke-linecap="round" />
          <rect x="27" y="100" width="61" height="8" rx="4" fill="#286f68" opacity=".22" />
          <rect x="35" y="118" width="45" height="7" rx="3.5" fill="#286f68" opacity=".13" />
          </g>
        </g>

        <g class="forgetting-chart" transform="translate(255 461)">
          <path d="M0 75 H453 M0 75 V0" fill="none" stroke="#286f68" stroke-opacity=".2" stroke-width="2" />
          <path d="M0 12 C27 30 47 51 78 66 L78 29 C106 43 129 57 158 68 L158 35 C188 46 211 58 240 69 L240 42 C273 51 296 61 326 69 L326 48 C365 56 397 64 453 70" fill="none" stroke="url(#systemGold)" stroke-width="5" stroke-linecap="round" stroke-linejoin="round" class="memory-curve" />
          <g fill="#f6e4a1" stroke="#b18737" stroke-width="2">
            <circle cx="78" cy="29" r="6" />
            <circle cx="158" cy="35" r="6" />
            <circle cx="240" cy="42" r="6" />
            <circle cx="326" cy="48" r="6" />
          </g>
          <g stroke="#286f68" stroke-opacity=".12" stroke-width="1">
            <path d="M0 50 H453 M0 25 H453" />
            <path d="M113 0 V75 M226 0 V75 M339 0 V75" />
          </g>
        </g>

        <g class="system-sparks" filter="url(#systemGlow)">
          <path d="M721 105 l5 13 13 5-13 5-5 13-5-13-13-5 13-5z" fill="#f2d26e" />
          <circle cx="238" cy="111" r="6" fill="#4a958b" />
          <circle cx="735" cy="427" r="5" fill="#4a958b" />
        </g>
      </g>

      <rect x="24" y="24" width="772" height="572" rx="62" fill="none" stroke="#286f68" stroke-opacity=".14" stroke-width="1.5" />
    </svg>
  </div>
</template>

<style scoped>
.system-art { width: min(100%, 820px); height: 100%; }
svg { width: 100%; height: 100%; overflow: visible; }
.system-halo { animation: system-breathe 5s ease-in-out infinite; transform-origin: 394px 292px; }
.level-node { animation: level-pulse 4.5s ease-in-out infinite; transform-box: fill-box; transform-origin: center; }
.level-rail > g:nth-of-type(2) .level-node { animation-delay: -.7s; }
.level-rail > g:nth-of-type(3) .level-node { animation-delay: -1.4s; }
.level-rail > g:nth-of-type(4) .level-node { animation-delay: -2.1s; }
.level-rail > g:nth-of-type(5) .level-node { animation-delay: -2.8s; }
.core-cards { animation: core-float 5.8s ease-in-out infinite; transform-origin: 394px 283px; }
.grammar-tree path { stroke-dasharray: 6 7; animation: grammar-flow 5s linear infinite; }
.knowledge-network > g:first-child path { stroke-dasharray: 5 8; animation: network-flow 6s linear infinite; }
.knowledge-network > g:nth-child(2) circle { animation: node-breathe 4s ease-in-out infinite; transform-box: fill-box; transform-origin: center; }
.network-orbit { animation: network-spin 16s linear infinite; transform-origin: 600px 228px; }
.network-orbit--slow { animation-duration: 22s; animation-direction: reverse; transform-origin: 620px 386px; }
.card-stack { animation: stack-float 5.1s ease-in-out infinite; transform-box: fill-box; transform-origin: center; }
.memory-curve { stroke-dasharray: 720; stroke-dashoffset: 720; animation: curve-draw 3.2s cubic-bezier(.2,.8,.2,1) .2s forwards, curve-glow 3.8s ease-in-out 3.4s infinite; }
.system-sparks { animation: system-spark 3.2s ease-in-out infinite; transform-origin: center; }
@keyframes system-breathe { 0%, 100% { opacity: .62; transform: scale(.92); } 50% { opacity: 1; transform: scale(1.08); } }
@keyframes level-pulse { 0%, 100% { transform: scale(1); } 50% { transform: scale(1.08); } }
@keyframes core-float { 0%, 100% { transform: translateY(0) rotate(-.2deg); } 50% { transform: translateY(-8px) rotate(.2deg); } }
@keyframes grammar-flow { to { stroke-dashoffset: -52; } }
@keyframes network-flow { to { stroke-dashoffset: -52; } }
@keyframes node-breathe { 0%, 100% { transform: scale(.96); } 50% { transform: scale(1.08); } }
@keyframes network-spin { to { transform: rotate(360deg); } }
@keyframes stack-float { 0%, 100% { transform: translateY(0) rotate(-1deg); } 50% { transform: translateY(-9px) rotate(2deg); } }
@keyframes curve-draw { to { stroke-dashoffset: 0; } }
@keyframes curve-glow { 0%, 100% { opacity: .72; } 50% { opacity: 1; } }
@keyframes system-spark { 0%, 100% { opacity: .55; transform: scale(.92); } 50% { opacity: 1; transform: scale(1.12); } }
</style>
