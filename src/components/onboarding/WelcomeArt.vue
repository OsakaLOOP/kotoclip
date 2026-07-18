<template>
  <div class="welcome-art" aria-hidden="true">
    <svg viewBox="0 0 720 610" role="img">
      <defs>
        <linearGradient id="welcomePortal" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#6a61e8" />
          <stop offset="1" stop-color="#b66ad1" />
        </linearGradient>
        <linearGradient id="welcomePage" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#fffef9" />
          <stop offset="1" stop-color="#e9e3ff" />
        </linearGradient>
        <linearGradient id="welcomeToken" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#ffffff" />
          <stop offset="1" stop-color="#eee8ff" />
        </linearGradient>
        <filter id="welcomeShadow" x="-30%" y="-30%" width="160%" height="180%">
          <feDropShadow dx="0" dy="22" stdDeviation="24" flood-color="#3d336d" flood-opacity=".16" />
        </filter>
      </defs>

      <circle cx="379" cy="294" r="226" fill="url(#welcomePortal)" opacity=".12" />
      <circle cx="379" cy="294" r="176" fill="none" stroke="#6a61e8" stroke-opacity=".16" stroke-width="1.5" stroke-dasharray="5 13" class="orbit" />
      <circle cx="379" cy="294" r="206" fill="none" stroke="#ffb14c" stroke-opacity=".11" stroke-width="1.2" stroke-dasharray="2 18" class="orbit orbit-slow" />
      <path d="M212 436 C292 346 332 292 375 170 C419 294 467 355 552 436 Z" fill="#4d4778" opacity=".1" />
      <path d="M263 430 C325 366 353 320 379 242 C407 324 444 373 503 430 Z" fill="#5a55c8" opacity=".2" />

      <g filter="url(#welcomeShadow)">
        <g transform="translate(142 268) rotate(-9)">
          <g class="floating-token token-one">
            <rect x="-47" y="-23" width="94" height="46" rx="17" fill="url(#welcomeToken)" />
            <rect x="-29" y="-7" width="18" height="14" rx="4" fill="#ff9a72" />
            <rect x="-5" y="-7" width="35" height="6" rx="3" fill="#6a61e8" opacity=".45" />
            <rect x="-5" y="4" width="25" height="5" rx="2.5" fill="#6a61e8" opacity=".2" />
          </g>
        </g>
        <g transform="translate(585 349) rotate(7)">
          <g class="floating-token token-two">
            <rect x="-42" y="-50" width="84" height="100" rx="22" fill="url(#welcomeToken)" />
            <circle cy="-20" r="13" fill="#ffcf63" />
            <path d="M-4 -20 H4 M0 -24 V-16" stroke="#6158c5" stroke-width="3" stroke-linecap="round" />
            <rect x="-24" y="8" width="48" height="6" rx="3" fill="#6a61e8" opacity=".27" />
            <rect x="-18" y="22" width="36" height="6" rx="3" fill="#6a61e8" opacity=".16" />
          </g>
        </g>
        <g transform="translate(224 158) rotate(5)">
          <g class="floating-token token-three">
            <path d="M-34 -23 H34 V18 H9 L-8 31 V18 H-34 Z" fill="url(#welcomeToken)" />
            <circle cx="-15" cy="-3" r="4" fill="#6a61e8" />
            <circle cy="-3" r="4" fill="#8f75df" />
            <circle cx="15" cy="-3" r="4" fill="#ffb14c" />
          </g>
        </g>
        <g transform="translate(548 465) rotate(-6)">
          <g class="floating-token token-four">
            <path d="M-34 -29 H-20 V-17 M-34 -29 V29 H-20 V17 M34 -29 H20 V-17 M34 -29 V29 H20 V17" fill="none" stroke="#7959b9" stroke-width="7" stroke-linecap="round" />
            <circle r="9" fill="#ffbf4d" />
          </g>
        </g>
      </g>

      <g filter="url(#welcomeShadow)" class="book">
        <path d="M170 390 C246 355 310 365 360 405 L360 521 C302 476 237 465 170 492 Z" fill="#d8d0ec" opacity=".55" transform="translate(-11 12)" />
        <path d="M550 390 C474 355 410 365 360 405 L360 521 C418 476 483 465 550 492 Z" fill="#cfc4e7" opacity=".5" transform="translate(11 12)" />
        <path d="M158 370 C234 337 306 346 360 395 L360 511 C303 466 236 452 158 482 Z" fill="url(#welcomePage)" />
        <path d="M562 370 C486 337 414 346 360 395 L360 511 C417 466 484 452 562 482 Z" fill="#f9f6ff" />
        <path d="M360 395 C309 358 247 348 178 369" fill="none" stroke="#6a61e8" stroke-opacity=".2" stroke-width="3" />
        <path d="M360 395 C411 358 473 348 542 369" fill="none" stroke="#6a61e8" stroke-opacity=".2" stroke-width="3" />
        <path d="M360 397 L360 507" stroke="#514a91" stroke-opacity=".25" stroke-width="3" />
        <path d="M194 398 C247 379 296 385 333 410 M194 427 C247 408 296 414 333 439" fill="none" stroke="#69618b" stroke-opacity=".18" stroke-width="7" stroke-linecap="round" />
        <path d="M526 398 C473 379 424 385 387 410 M526 427 C473 408 424 414 387 439" fill="none" stroke="#69618b" stroke-opacity=".18" stroke-width="7" stroke-linecap="round" />
        <path d="M194 456 C240 442 282 445 316 462 M526 456 C480 442 438 445 404 462" fill="none" stroke="#69618b" stroke-opacity=".13" stroke-width="6" stroke-linecap="round" />
        <path d="M158 370 C173 395 188 401 211 403" fill="none" stroke="#fff" stroke-opacity=".75" stroke-width="3" />
        <path d="M562 370 C547 395 532 401 509 403" fill="none" stroke="#fff" stroke-opacity=".75" stroke-width="3" />
      </g>

      <g class="clip">
        <path d="M445 184 C472 163 507 179 507 208 L507 294 C507 328 463 344 440 320 C426 305 430 281 448 271 L481 252" fill="none" stroke="#ffb14c" stroke-width="16" stroke-linecap="round" />
        <path d="M454 213 L454 276 C454 290 471 297 481 288" fill="none" stroke="#7959b9" stroke-width="9" stroke-linecap="round" />
      </g>

      <g class="sparkles" fill="#f6b84b">
        <path d="M204 206 l6 15 15 6-15 6-6 15-6-15-15-6 15-6z" />
        <circle cx="545" cy="260" r="7" fill="#7959b9" />
        <circle cx="244" cy="294" r="4" fill="#ff8674" />
      </g>
    </svg>
  </div>
</template>

<style scoped>
.welcome-art { width: min(100%, 720px); height: 100%; }
svg { width: 100%; height: 100%; overflow: visible; }
.book { animation: book-breathe 5.8s ease-in-out infinite; transform-origin: 360px 440px; }
.clip { animation: clip-float 4.6s ease-in-out infinite; transform-origin: center; }
.orbit { animation: orbit-spin 30s linear infinite; transform-origin: 379px 294px; }
.orbit-slow { animation-duration: 46s; animation-direction: reverse; }
.floating-token { animation: token-float 5s ease-in-out infinite; transform-box: fill-box; transform-origin: center; }
.token-two { animation-delay: -1.2s; }
.token-three { animation-delay: -2.4s; }
.token-four { animation-delay: -3.4s; }
.sparkles { animation: sparkle 3.2s ease-in-out infinite; transform-origin: center; }
@keyframes book-breathe { 0%, 100% { transform: translateY(0) rotate(-.3deg); } 50% { transform: translateY(-8px) rotate(.3deg); } }
@keyframes clip-float { 0%, 100% { transform: translateY(0) rotate(-4deg); } 50% { transform: translateY(-12px) rotate(3deg); } }
@keyframes orbit-spin { to { transform: rotate(360deg); } }
@keyframes token-float { 0%, 100% { transform: translateY(0) rotate(-1deg); } 50% { transform: translateY(-8px) rotate(1.5deg); } }
@keyframes sparkle { 0%, 100% { opacity: .55; transform: scale(.95); } 50% { opacity: 1; transform: scale(1.08); } }
</style>
