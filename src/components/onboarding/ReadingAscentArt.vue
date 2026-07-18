<script setup lang="ts">
defineProps<{
  prompting?: boolean;
}>();

const emit = defineEmits<{
  claim: [];
}>();
</script>

<template>
  <div class="ascent-art">
    <svg class="mountain-scene" viewBox="0 0 820 620" role="img" aria-label="借助阅读工具攀登雪峰的插画">
      <defs>
        <linearGradient id="ascentSky" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#f9fbf8" />
          <stop offset=".52" stop-color="#dcebea" />
          <stop offset="1" stop-color="#c9dce1" />
        </linearGradient>
        <radialGradient id="ascentSun" cx="50%" cy="50%" r="50%">
          <stop offset="0" stop-color="#fff9c7" stop-opacity=".96" />
          <stop offset=".28" stop-color="#ffe98c" stop-opacity=".62" />
          <stop offset="1" stop-color="#ffe98c" stop-opacity="0" />
        </radialGradient>
        <linearGradient id="ascentFar" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#9db8bd" />
          <stop offset="1" stop-color="#d8e4e1" />
        </linearGradient>
        <linearGradient id="ascentMountain" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#536e79" />
          <stop offset=".55" stop-color="#324d5d" />
          <stop offset="1" stop-color="#223747" />
        </linearGradient>
        <linearGradient id="ascentMountainLit" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#75919a" />
          <stop offset="1" stop-color="#3d5e6b" />
        </linearGradient>
        <linearGradient id="ascentSnow" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#ffffff" />
          <stop offset="1" stop-color="#dbe8e9" />
        </linearGradient>
        <linearGradient id="ascentTrail" x1="0" y1="0" x2="1" y2="0">
          <stop offset="0" stop-color="#efb95e" />
          <stop offset=".65" stop-color="#f5d17a" />
          <stop offset="1" stop-color="#fff2ac" />
        </linearGradient>
        <pattern id="ascentRockTexture" width="26" height="26" patternUnits="userSpaceOnUse" patternTransform="rotate(18)">
          <path d="M0 13 H26" stroke="#f4fbfa" stroke-opacity=".075" stroke-width="1" />
          <circle cx="7" cy="6" r="1.2" fill="#f4fbfa" fill-opacity=".09" />
          <circle cx="21" cy="20" r=".8" fill="#f4fbfa" fill-opacity=".08" />
        </pattern>
        <filter id="ascentCardShadow" x="-40%" y="-40%" width="180%" height="190%">
          <feDropShadow dx="0" dy="14" stdDeviation="18" flood-color="#17323f" flood-opacity=".15" />
        </filter>
        <filter id="ascentGlow" x="-80%" y="-80%" width="260%" height="260%">
          <feGaussianBlur stdDeviation="8" result="blur" />
          <feMerge><feMergeNode in="blur" /><feMergeNode in="SourceGraphic" /></feMerge>
        </filter>
        <clipPath id="ascentFrameClip">
          <rect x="24" y="24" width="772" height="572" rx="62" />
        </clipPath>
      </defs>

      <g clip-path="url(#ascentFrameClip)">
        <rect x="24" y="24" width="772" height="572" rx="62" fill="url(#ascentSky)" />
        <circle cx="642" cy="127" r="112" fill="url(#ascentSun)" class="sun-halo" />

        <g class="sky-lines" fill="none" stroke="#4f7180" stroke-opacity=".12" stroke-width="1.2">
          <path d="M72 122 C184 63 302 72 399 124" />
          <path d="M64 152 C201 92 328 107 421 159" />
          <path d="M503 70 C611 40 710 69 771 125" />
        </g>

        <g class="clouds" fill="#fff" fill-opacity=".58">
          <path d="M80 212 C111 184 145 191 157 215 C181 202 211 218 213 242 H72 C71 231 73 220 80 212 Z" />
          <path d="M610 219 C634 196 666 201 677 223 C697 212 727 225 732 246 H598 C598 235 602 226 610 219 Z" />
        </g>

        <path d="M24 447 L173 301 L267 397 L357 262 L468 421 L579 277 L796 466 V596 H24 Z" fill="url(#ascentFar)" opacity=".72" />
        <path d="M24 511 L186 370 L316 512 L447 354 L558 488 L687 348 L796 465 V596 H24 Z" fill="#819ca2" opacity=".38" />

        <g class="main-mountain">
          <path d="M82 596 L618 112 L796 596 Z" fill="url(#ascentMountain)" />
          <path d="M82 596 L618 112 L522 596 Z" fill="url(#ascentMountainLit)" />
          <path d="M256 596 L618 112 L433 596 Z" fill="#78949a" opacity=".2" />
          <path d="M618 112 L544 245 L589 225 L614 280 L650 233 L688 277 L681 205 Z" fill="url(#ascentSnow)" />
          <path d="M618 112 L544 245 L589 225 L614 280 Z" fill="#ffffff" />
          <path d="M618 112 L681 205 L650 233 Z" fill="#e0ebec" />
          <path d="M82 596 L618 112 L796 596 Z" fill="url(#ascentRockTexture)" />

          <g fill="none" stroke="#d8e9e8" stroke-opacity=".18" stroke-width="2">
            <path d="M245 540 L385 389 L463 397" />
            <path d="M398 596 L470 442 L560 339" />
            <path d="M585 492 L647 365 L733 507" />
            <path d="M508 297 L558 315 L585 259" />
          </g>
        </g>

        <path
          class="trail"
          d="M145 560 C231 520 290 536 329 491 C365 450 347 422 402 397 C454 373 457 337 504 316 C548 295 562 255 593 217"
          fill="none"
          stroke="#1f3440"
          stroke-opacity=".24"
          stroke-width="14"
          stroke-linecap="round"
        />
        <path
          class="trail-light"
          d="M145 560 C231 520 290 536 329 491 C365 450 347 422 402 397 C454 373 457 337 504 316 C548 295 562 255 593 217"
          fill="none"
          stroke="url(#ascentTrail)"
          stroke-width="5"
          stroke-linecap="round"
          stroke-dasharray="10 13"
        />

        <g class="trail-markers" filter="url(#ascentCardShadow)">
          <g transform="translate(216 490)">
            <g class="trail-marker">
            <circle r="25" fill="#f6f2df" />
            <rect x="-12" y="-9" width="9" height="9" rx="2" fill="#d99651" />
            <rect x="2" y="-9" width="12" height="9" rx="2" fill="#315d73" />
            <rect x="-9" y="4" width="20" height="7" rx="3.5" fill="#668d95" />
            </g>
          </g>
          <g transform="translate(360 430)">
            <g class="trail-marker">
            <circle r="25" fill="#eef6f4" />
            <path d="M-14 -10 C-6 -13 -1 -9 0 -5 C1 -9 6 -13 14 -10 V12 C7 9 2 11 0 15 C-2 11 -7 9 -14 12 Z" fill="#315d73" />
            <path d="M0 -5 V15" stroke="#eef6f4" stroke-width="2" />
            </g>
          </g>
          <g transform="translate(480 341)">
            <g class="trail-marker">
            <circle r="25" fill="#f4eff8" />
            <path d="M-15 -12 H-7 V-5 M-15 -12 V12 H-7 V5 M15 -12 H7 V-5 M15 -12 V12 H7 V5" fill="none" stroke="#76538f" stroke-width="4" stroke-linecap="round" />
            <circle cx="0" cy="0" r="5" fill="#efb95e" />
            </g>
          </g>
          <g transform="translate(564 259)">
            <g class="trail-marker">
            <circle r="25" fill="#fff4d4" />
            <path d="M-13 -9 H13 V8 H3 L-4 14 V8 H-13 Z" fill="#315d73" />
            <path d="M0 -5 l2.4 5.2 5.6.6-4.2 3.8 1.2 5.5L0 7.2-5 10l1.2-5.4L-8 .8-2.4.2Z" fill="#f6c765" />
            </g>
          </g>
        </g>

        <g transform="translate(120 332) rotate(-7)">
          <g class="trekking-poles">
          <path d="M36 9 L9 173 M70 15 L51 176" stroke="#263e4c" stroke-width="7" stroke-linecap="round" />
          <path d="M27 8 H49 M59 14 H81" stroke="#d99b58" stroke-width="9" stroke-linecap="round" />
          <path d="M4 173 H16 M45 176 H58" stroke="#263e4c" stroke-width="6" stroke-linecap="round" />
          <path d="M34 34 C11 51 11 77 25 94 M69 38 C48 54 46 79 58 95" fill="none" stroke="#d99b58" stroke-width="3" stroke-linecap="round" />
          </g>
        </g>

        <g transform="translate(294 281) rotate(8)" filter="url(#ascentCardShadow)">
          <g class="sunglasses">
          <path d="M8 24 C14 1 48 0 59 17 L63 22 C71 11 80 9 89 11 L87 21 C77 18 71 20 65 28 C59 45 47 55 31 53 C13 52 3 41 8 24 Z" fill="#263e4c" />
          <path d="M73 24 C79 1 113 0 124 17 C134 34 123 51 104 53 C86 54 68 42 73 24 Z" fill="#263e4c" />
          <path d="M17 23 C23 10 43 8 51 19 C55 33 44 43 31 43 C19 43 11 35 17 23 Z M82 23 C88 10 108 8 116 19 C120 33 109 43 96 43 C84 43 76 35 82 23 Z" fill="#8fbac2" opacity=".78" />
          <path d="M58 22 C64 18 70 18 76 22" fill="none" stroke="#263e4c" stroke-width="7" />
          <path d="M20 20 L48 37 M85 20 L113 37" stroke="#e8fbfb" stroke-opacity=".45" stroke-width="3" />
          </g>
        </g>

        <g transform="translate(385 444)">
          <g class="climber">
          <ellipse cx="0" cy="69" rx="42" ry="10" fill="#142936" opacity=".18" />
          <circle cx="0" cy="-31" r="17" fill="#162f3d" />
          <path d="M-15 -17 C-36 3 -32 31 -21 44 L-9 25 L7 44 C28 20 26 -4 15 -18 Z" fill="#e29d5b" />
          <path d="M-17 -12 C-41 -3 -47 18 -37 35 L-22 28 L-9 4 Z" fill="#5a3f77" />
          <path d="M-18 37 L-32 69 M7 39 L23 67" stroke="#173442" stroke-width="10" stroke-linecap="round" />
          <path d="M-10 -1 L-34 26 M13 -3 L31 21" stroke="#173442" stroke-width="8" stroke-linecap="round" />
          <path d="M-35 26 L-48 70 M31 21 L41 65" stroke="#654b36" stroke-width="4" stroke-linecap="round" />
          <path d="M-12 -35 H12 M-8 -31 H-1 M3 -31 H10" stroke="#eaf9f7" stroke-width="3" stroke-linecap="round" />
          </g>
        </g>

        <g filter="url(#ascentGlow)" transform="translate(619 105)">
          <g class="summit-spark">
          <path d="M0 -23 L6 -6 L23 0 L6 6 L0 23 L-6 6 L-23 0 L-6 -6 Z" fill="#fff4a6" />
          <circle r="5" fill="#fff" />
          </g>
        </g>
      </g>

      <rect x="24" y="24" width="772" height="572" rx="62" fill="none" stroke="#315d73" stroke-opacity=".14" stroke-width="1.5" />
    </svg>

    <button
      class="reward-pickup"
      :class="{ prompting }"
      type="button"
      aria-label="奖励自己三口一个巧乐兹，并前往兴趣激励页"
      @click="emit('claim')"
    >
      <span class="reward-ring ring-one" aria-hidden="true"></span>
      <span class="reward-ring ring-two" aria-hidden="true"></span>
      <span class="reward-object" aria-hidden="true">
        <svg viewBox="0 0 112 142">
          <defs>
            <linearGradient id="rewardPurple" x1="0" y1="0" x2="1" y2="1">
              <stop offset="0" stop-color="#7b4aa0" />
              <stop offset="1" stop-color="#4f2e76" />
            </linearGradient>
            <linearGradient id="rewardYellow" x1="0" y1="0" x2="1" y2="1">
              <stop offset="0" stop-color="#ffe36c" />
              <stop offset="1" stop-color="#f3b83d" />
            </linearGradient>
          </defs>
          <path d="M19 8 H93 L87 99 C84 121 73 135 56 137 C39 135 28 121 25 99 Z" fill="url(#rewardPurple)" />
          <path d="M19 8 H93 L91 43 C75 35 50 54 23 41 Z" fill="url(#rewardYellow)" />
          <path d="M25 75 C46 61 67 61 89 75 L86 103 C66 91 46 91 27 103 Z" fill="#ffd956" />
          <path d="M42 17 C47 29 62 34 81 25" fill="none" stroke="#fff5b3" stroke-opacity=".75" stroke-width="5" stroke-linecap="round" />
          <path d="M40 78 L72 101 M74 77 L40 103" stroke="#6e438f" stroke-width="4" stroke-linecap="round" opacity=".72" />
          <path d="M30 9 Q56 -3 82 9" fill="#f6c84f" />
          <path d="M51 42 C55 34 66 36 67 44 C68 52 58 58 52 65 C45 58 36 52 37 44 C38 36 48 34 51 42 Z" fill="#fff1a3" opacity=".9" />
        </svg>
      </span>
      <span class="reward-copy">
        <small>还要奖励自己</small>
        <strong>三口一个巧乐兹</strong>
      </span>
    </button>
  </div>
</template>

<style scoped>
.ascent-art { position: relative; width: min(100%, 820px); height: 100%; }
.mountain-scene { width: 100%; height: 100%; overflow: visible; }
.sun-halo { animation: sun-pulse 4.8s ease-in-out infinite; transform-origin: 642px 127px; }
.clouds { animation: cloud-drift 9s ease-in-out infinite alternate; }
.trail-light { animation: trail-flow 3.6s linear infinite; }
.trail-marker { animation: marker-breathe 4s ease-in-out infinite; transform-box: fill-box; transform-origin: center; }
.trail-markers > g:nth-child(2) .trail-marker { animation-delay: -.8s; }
.trail-markers > g:nth-child(3) .trail-marker { animation-delay: -1.6s; }
.trail-markers > g:nth-child(4) .trail-marker { animation-delay: -2.4s; }
.trekking-poles { animation: equipment-float 5.2s ease-in-out infinite; transform-box: fill-box; transform-origin: center; }
.sunglasses { animation: glasses-float 4.4s ease-in-out infinite; transform-box: fill-box; transform-origin: center; }
.climber { animation: climber-step 2.2s ease-in-out infinite; transform-box: fill-box; transform-origin: bottom; }
.summit-spark { animation: summit-shine 2.8s ease-in-out infinite; transform-box: fill-box; transform-origin: center; }

.reward-pickup {
  position: absolute;
  z-index: 5;
  right: 1.8%;
  top: 47%;
  display: flex;
  align-items: center;
  gap: 9px;
  width: 192px;
  min-height: 105px;
  padding: 11px 12px 11px 9px;
  border: 1px solid rgba(104, 62, 137, .16);
  border-radius: 28px;
  color: #513562;
  background: rgba(255, 250, 239, .82);
  box-shadow: 0 18px 45px rgba(68, 45, 82, .14);
  backdrop-filter: blur(12px);
  cursor: pointer;
  transform: rotate(4.5deg);
  transition: transform 260ms cubic-bezier(.2,.8,.2,1), box-shadow 260ms ease;
}

.reward-pickup:hover,
.reward-pickup:focus-visible {
  transform: rotate(1deg) translateY(-5px) scale(1.025);
  box-shadow: 0 22px 52px rgba(68, 45, 82, .22);
}

.reward-pickup:focus-visible { outline: 3px solid rgba(123, 74, 160, .35); outline-offset: 5px; }
.reward-object { flex: 0 0 auto; width: 57px; height: 78px; filter: drop-shadow(0 9px 9px rgba(54, 30, 73, .2)); }
.reward-object svg { width: 100%; height: 100%; }
.reward-copy { display: grid; gap: 2px; text-align: left; line-height: 1.3; }
.reward-copy small { color: #8a6c96; font-size: .62rem; }
.reward-copy strong { font-size: .74rem; font-weight: 780; }

.reward-ring {
  position: absolute;
  inset: -9px;
  z-index: -1;
  border: 1px solid rgba(123, 74, 160, .32);
  border-radius: 34px;
  animation: reward-ring 2.8s ease-out infinite;
}
.ring-two { animation-delay: -1.4s; }
.reward-pickup.prompting { animation: reward-nudge 680ms cubic-bezier(.2,.82,.2,1) both; }
.reward-pickup.prompting .reward-ring { border-width: 2px; border-color: rgba(123, 74, 160, .58); }

@keyframes sun-pulse { 0%, 100% { opacity: .76; transform: scale(.94); } 50% { opacity: 1; transform: scale(1.06); } }
@keyframes cloud-drift { from { transform: translateX(-5px); } to { transform: translateX(11px); } }
@keyframes trail-flow { to { stroke-dashoffset: -46; } }
@keyframes marker-breathe { 0%, 100% { transform: translateY(0) scale(1); } 50% { transform: translateY(-5px) scale(1.04); } }
@keyframes equipment-float { 0%, 100% { transform: translateY(0) rotate(-2deg); } 50% { transform: translateY(-10px) rotate(2deg); } }
@keyframes glasses-float { 0%, 100% { transform: translateY(0) rotate(-1deg); } 50% { transform: translateY(-8px) rotate(2deg); } }
@keyframes climber-step { 0%, 100% { transform: translateY(0) rotate(-.6deg); } 50% { transform: translateY(-3px) rotate(.8deg); } }
@keyframes summit-shine { 0%, 100% { opacity: .58; transform: scale(.68) rotate(0); } 50% { opacity: 1; transform: scale(1.14) rotate(45deg); } }
@keyframes reward-ring { 0% { opacity: .72; transform: scale(.92); } 75%, 100% { opacity: 0; transform: scale(1.14); } }
@keyframes reward-nudge { 0%, 100% { transform: rotate(4.5deg); } 32% { transform: rotate(-2deg) translateY(-7px) scale(1.04); } 62% { transform: rotate(6deg) translateY(-2px); } }

@media (max-width: 620px) {
  .reward-pickup { right: 0; top: 44%; width: 150px; min-height: 82px; border-radius: 23px; }
  .reward-object { width: 44px; height: 62px; }
  .reward-copy strong { font-size: .64rem; }
  .reward-copy small { font-size: .55rem; }
}
</style>
