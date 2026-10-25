// Fig: Sentinel's sampler as a four-pen strip-chart recorder.
// Paper runs from the supply roll under a bridge of four pens to the take-up
// roll. Each pen writes one stream: CPU, memory, network, disk. The selector
// knob is the view on screen: pens for streams that view doesn't need lift off
// the paper, because Sentinel only samples what you're looking at. The toggle
// switches between the 1 s default and the 500 ms high-refresh mode, which
// doubles how much paper each second uses.
import * as G from "./kit/lathe.mjs";
import { emit, k, tools } from "./common.mjs";

const WIDTH = 640;
const HEIGHT = 380;

const PLATE = { x: -112, y: -66, w: 224, d: 150, r: 9 };
const PLATE_H = 12;
const FOOT_H = 5;

const TABLE = { x: -72, y: -46, w: 144, d: 92, r: 3 };
const TABLE_TOP = 16;
const PAPER = { x: -72, y: -42, w: 144, d: 84, r: 0.6 };
const PAPER_Z = TABLE_TOP + 0.4;
const ROLL_R = 8;
const ROLL_X = 82;
const ROLL_Z = PAPER_Z - ROLL_R;
const ROLL_Y = 46;

const LANES = [-31.5, -10.5, 10.5, 31.5]; // centres, 21 wide each, across y
const LANE_HALF = 8.5;
export const STREAMS = ["CPU", "memory", "network", "disk"];
const VIEWS = [
  { name: "Processes", uses: [0, 1] },
  { name: "Network", uses: [2] },
  { name: "CPU and memory", uses: [0, 1] },
  { name: "Storage", uses: [3] },
];
const PEN_X = 4;
const BEAM_Z = 40;
const POST_Y = 56;
const PEN_DOWN = PAPER_Z + 0.2;
const PEN_UP = PAPER_Z + 4.5;

const BOX = { x: 44, y: 54, w: 42, d: 24, r: 3 };
const BOX_TOP = 14;
const KNOB = [58, 66];
const KNOB_R = 6.5;
const TOGGLE = [76, 66];

const P = k.fitProjection(
  [...k.boxCorners(PLATE, -PLATE_H - FOOT_H, 0), [PEN_X, -POST_Y, BEAM_Z + 8], [PEN_X, POST_Y, BEAM_Z + 8], [-ROLL_X, 0, PAPER_Z + 2], [ROLL_X, 0, PAPER_Z + 2]],
  WIDTH,
  HEIGHT,
  { pad: 18, azimuth: 34 },
);
const T = tools(P);
const { at, S, L, D } = T;

function base() {
  const out = [T.basePlate(PLATE, PLATE_H, FOOT_H, { footInset: 22 })];
  out.push(T.ruler({ x: -100, y: -60, w: 7, d: 120, r: 1 }, 0, "y"));
  out.push(T.labelPlate({ x: -98, y: 58, w: 46, d: 14, r: 1.2 }, 0));
  return out.join("");
}

// The table the paper runs over, on four blocks.
function table() {
  const out = [T.halo(TABLE, 0, TABLE_TOP)];
  for (const [x, y] of T.byDepth(k.corners(TABLE, 10))) {
    out.push(S(k.slabOf({ x: x - 5, y: y - 5, w: 10, d: 10, r: 1.5 }, 0, TABLE_TOP - 4, P, 3, 0.4), { tone: "lo" }));
  }
  out.push(S(k.slabOf(TABLE, TABLE_TOP - 4, 4, P, 4, 0.8), { tone: "mid" }));
  out.push(D(k.corners(TABLE, 4).map(([x, y]) => at([x, y, TABLE_TOP])), { size: 0.45 }));
  return out.join("");
}

function roll(x, front) {
  const out = [];
  const F = G.frameAlong([x, -ROLL_Y + 6, ROLL_Z], [0, 1, 0]);
  const len = 2 * ROLL_Y - 12;
  const spindle = G.frameAlong([x, -ROLL_Y - 2, ROLL_Z], [0, 1, 0]);
  // Bearing blocks at both ends, the roll of paper, flanges and the spindle.
  const block = (y) => S(k.slabOf({ x: x - 5, y: y - 3, w: 10, d: 6, r: 1 }, 0, ROLL_Z + 4, P, 3, 0.4), { tone: "mid" });
  out.push(block(-ROLL_Y - 3));
  out.push(S(G.disc(0, 2 * ROLL_Y + 4, 2, spindle, P), { tone: "mid" }));
  out.push(S(G.disc(-2, 0, ROLL_R + 3, F, P, { bevel: 0.3 }), { tone: "mid" }));
  out.push(S(G.solidOf([[0, ROLL_R], [len, ROLL_R]], F, P), { tone: "hi", crease: "none" }));
  out.push(L(G.arcOf(F, len * 0.25, ROLL_R, P) + G.arcOf(F, len * 0.5, ROLL_R, P) + G.arcOf(F, len * 0.75, ROLL_R, P), { tone: "faint" }));
  out.push(S(G.disc(len, len + 2, ROLL_R + 3, F, P, { bevel: 0.3 }), { tone: "mid" }));
  out.push(block(ROLL_Y + 3));
  out.push(D([at([x, ROLL_Y + 3, ROLL_Z + 4])], { size: 0.45 }));
  if (front) {
    // Crank on the take-up roll.
    out.push(S(G.disc(2 * ROLL_Y + 4, 2 * ROLL_Y + 6, 3.2, spindle, P), { tone: "mid" }));
  }
  return out.join("");
}

function paper() {
  const out = [];
  out.push(S(k.slabOf(PAPER, PAPER_Z - 0.3, 0.3, P, 2), { tone: "hi", crease: "none", flat: true }));
  // Lane borders, edge to edge.
  const borders = [-21, 0, 21];
  out.push(L(borders.map((y) => k.lineOnTop([PAPER.x, y], [PAPER.x + PAPER.w, y], PAPER_Z, P)).join(""), { tone: "faint" }));
  out.push(`<g data-part="grid"></g>`);
  out.push(`<g data-part="traces"></g>`);
  return out.join("");
}

function post(y) {
  const out = [];
  out.push(S(k.cylinder(PEN_X, y, 6, 0, 3, P, 28), { tone: "lo" }));
  out.push(S(k.cylinder(PEN_X, y, 3.8, 3, BEAM_Z - 3, P, 28), { tone: "mid", crease: "none" }));
  out.push(L([14, 28].map((z) => k.sideArc(PEN_X, y, 3.8, z, P, 16)).join(""), { tone: "faint" }));
  return out.join("");
}

function beam() {
  const out = [];
  const plan = { x: PEN_X - 6, y: -POST_Y - 5, w: 12, d: 2 * POST_Y + 10, r: 2 };
  out.push(S(k.slabOf(plan, BEAM_Z, 6, P, 4, 0.8), { tone: "mid" }));
  out.push(L(k.planOutline(k.insetPlan(plan, 2.2), BEAM_Z + 6, P, 4), { tone: "faint" }));
  out.push(D([-POST_Y, POST_Y].map((y) => at([PEN_X, y, BEAM_Z + 6])), { size: 0.5 }));
  // Four galvanometer heads on the beam, one per lane.
  for (const y of LANES) {
    out.push(S(k.cylinder(PEN_X, y, 4.6, BEAM_Z + 6, 4, P, 28, 0.4), { tone: "mid" }));
    out.push(L(k.knurl(PEN_X, y, 4.6, BEAM_Z + 6, BEAM_Z + 10, 26, P), { tone: "lo" }));
    out.push(D([at([PEN_X, y, BEAM_Z + 10])], { size: 0.5, tone: "hi" }));
  }
  return out.join("");
}

// A pen: a slider hanging from the beam's rail plus a tip, built at y = 0, z rest.
export function penSvg() {
  const out = [];
  out.push(S(k.slabOf({ x: PEN_X - 3.5, y: -3, w: 7, d: 6, r: 1 }, BEAM_Z - 7, 7, P, 3, 0.4), { tone: "mid" }));
  out.push(S(k.cylinder(PEN_X, 0, 1.5, PEN_DOWN + 5, BEAM_Z - 7 - PEN_DOWN - 5, P, 16), { tone: "mid", crease: "none" }));
  out.push(S(k.cylinder(PEN_X, 0, 2.2, PEN_DOWN + 1.2, 3.8, P, 20), { tone: "hi", lit: true }));
  out.push(S(k.cylinder(PEN_X, 0, 0.9, PEN_DOWN, 1.2, P, 12), { tone: "hi", crease: "none" }));
  return out.join("");
}

function controls() {
  const out = [T.halo(BOX, 0, BOX_TOP)];
  out.push(S(k.slabOf(BOX, 0, BOX_TOP, P, 5, 1), { tone: "mid" }));
  out.push(L(k.planOutline(k.insetPlan(BOX, 2.4), BOX_TOP, P, 5), { tone: "faint" }));
  out.push(D(k.corners(BOX, 3).map(([x, y]) => at([x, y, BOX_TOP])), { size: 0.42 }));
  const [x, y] = KNOB;
  const ticks = k.radialTicks(x, y, KNOB_R + 3.6, 4, 1, BOX_TOP, [1.6, 2.4], P, -Math.PI * 0.45, Math.PI * 1.2);
  out.push(L(ticks.major, { tone: "mid" }));
  out.push(S(k.cylinder(x, y, KNOB_R, BOX_TOP, 5, P, 32, 0.4), { tone: "mid" }));
  out.push(L(k.knurl(x, y, KNOB_R, BOX_TOP, BOX_TOP + 5, 34, P), { tone: "lo" }));
  out.push(`<path class="iso-line" data-tone="lit" data-free="end" data-part="knob" d="" style="stroke-width:1.1"/>`);
  out.push(L(k.ring(x, y, 1.4, BOX_TOP + 5, P, 14), { tone: "mid" }));
  out.push(D([at([x, y, BOX_TOP + 5])], { size: 0.5, tone: "hi" }));
  // Toggle: base nut and a lever that leans toward the selected mode.
  const [tx, ty] = TOGGLE;
  out.push(L(k.ring(tx, ty, 3.6, BOX_TOP, P, 20), { tone: "lo" }));
  out.push(S(k.cylinder(tx, ty, 2.4, BOX_TOP, 1.6, P, 20), { tone: "mid" }));
  out.push(`<g data-part="toggle"></g>`);
  return out.join("");
}

const svg = k.figureSvg({
  width: WIDTH,
  height: HEIGHT,
  label:
    "A four-pen strip-chart recorder. Paper runs from a supply roll under a bridge of four pens to a take-up roll. The pens write CPU, memory, network and disk in four lanes. A selector knob picks the view on screen, and the pens for streams that view doesn't need lift off the paper and stop writing. A toggle switches the paper between one sample a second and two.",
  body: [
    base(),
    roll(-ROLL_X, false),
    table(),
    paper(),
    post(-POST_Y),
    ...LANES.map((_, i) => `<g data-pen="${i}">${penSvg()}</g>`),
    beam(),
    post(POST_Y),
    roll(ROLL_X, true),
    controls(),
  ],
});

const live = String.raw`
const P = D.P, LANES = D.LANES, VIEWS = D.VIEWS, STREAMS = D.STREAMS;
const stage = root.querySelector(".iso-stage");
const svg = stage.querySelector("svg");
const q = (name) => svg.querySelector('[data-part="' + name + '"]');
const grid = q("grid"), traces = q("traces"), knob = q("knob"), toggle = q("toggle");
const pens = [...svg.querySelectorAll("[data-pen]")];
const readout = root.querySelector("[data-readout]");
const still = matchMedia("(prefers-reduced-motion: reduce)");
let view = 2, fast = false, knobAt = 2, toggleAt = 0;
let travel = 0, t = 0, frame = 0, last = 0, visible = false, touring = !still.matches, idleTimer = 0, tourClock = 0;
const pen = LANES.map(() => ({ y: 0, rate: 0, z: D.PEN_DOWN, value: 0 }));
const marks = LANES.map(() => []);
let sampleClock = 0;
const say = (text) => { if (readout.textContent !== text) readout.textContent = text; };
// Simulated signals, one per stream, as fractions of the lane.
const signal = [
  (s) => 0.35 + 0.25 * Math.sin(s * 0.9) + 0.18 * Math.sin(s * 2.7 + 1) + 0.08 * Math.sin(s * 7.1),
  (s) => 0.55 + 0.12 * Math.sin(s * 0.21) + 0.03 * Math.sin(s * 1.3),
  (s) => 0.15 + 0.7 * Math.max(0, Math.sin(s * 0.8)) ** 6 + 0.06 * Math.sin(s * 5.3),
  (s) => 0.2 + 0.45 * (Math.sin(s * 0.5) > 0.6 ? 1 : 0) * (0.6 + 0.4 * Math.sin(s * 3.1)),
];
const active = (i) => VIEWS[view].uses.includes(i);
function describe() {
  const used = VIEWS[view].uses.map((i) => STREAMS[i]).join(" and ");
  stage.setAttribute("aria-valuetext", VIEWS[view].name + " view: sampling " + used + " every " + (fast ? "500 ms" : "1 s") + ", everything else paused");
}
function drawKnob() {
  const [x, y] = D.KNOB, a = -Math.PI * 0.45 + knobAt * (Math.PI * 1.2) / 3;
  knob.setAttribute("d", k.lineOnTop([x + Math.sin(a) * 1.4, y - Math.cos(a) * 1.4], [x + Math.sin(a) * (D.KNOB_R - 1), y - Math.cos(a) * (D.KNOB_R - 1)], D.BOX_TOP + 5, P));
  const [tx, ty] = D.TOGGLE, lean = -1 + 2 * toggleAt;
  toggle.innerHTML = k.solidSvg(k.cylinder(tx + lean * 1.6, ty, 1, D.BOX_TOP + 1.6, 6, P, 14), { tone: "mid", crease: "none" })
    + k.solidSvg(k.cylinder(tx + lean * 2.2, ty, 1.5, D.BOX_TOP + 7.2, 1.6, P, 16), { tone: "hi" });
}
function draw() {
  // Paper grid: a time line every 12 units of paper, scrolling with it.
  let g = "";
  const off = travel % 12;
  for (let x = D.PAPER.x + off; x < D.PAPER.x + D.PAPER.w; x += 12) g += k.lineOnTop([x, D.PAPER.y], [x, D.PAPER.y + D.PAPER.d], D.PAPER_Z, P);
  grid.innerHTML = '<path class="iso-line" data-tone="faint" d="' + g + '"/>';
  let tr = "";
  marks.forEach((list, i) => {
    let d = "", run = [];
    const flush = () => { if (run.length > 1) d += k.onTop(run, D.PAPER_Z, P); run = []; };
    for (const m of list) {
      if (m === null) { flush(); continue; }
      const x = m[0] + (travel - m[2]);
      if (x > D.PAPER.x + D.PAPER.w - 0.5) { flush(); continue; }
      run.push([x, m[1]]);
    }
    flush();
    if (d) tr += '<path class="iso-line fig-trace" data-free="" data-stream="' + i + '" d="' + d + '"/>';
  });
  traces.innerHTML = tr;
  pens.forEach((el, i) => el.setAttribute("transform", k.translateAlong(P, [0, LANES[i] + pen[i].y, pen[i].z - D.PEN_DOWN])));
  drawKnob();
}
function tick(now) {
  const dt = last ? Math.min((now - last) / 1000, 1 / 30) : 1 / 60;
  last = now;
  const calm = still.matches;
  t += dt;
  if (touring) {
    tourClock += dt;
    const step = Math.floor(tourClock / 4.2);
    view = [2, 1, 0, 3][step % 4];
    fast = step % 8 >= 4;
  }
  knobAt = calm ? view : knobAt + (view - knobAt) * (1 - Math.exp(-dt / 0.18));
  toggleAt = calm ? (fast ? 1 : 0) : toggleAt + ((fast ? 1 : 0) - toggleAt) * (1 - Math.exp(-dt / 0.08));
  const speed = (fast ? 12 : 6) * (calm ? 0 : 1); // paper per second: 6 units a sample
  travel += speed * dt;
  sampleClock += dt;
  const period = fast ? 0.5 : 1;
  const sampleNow = sampleClock >= period;
  if (sampleNow) sampleClock = 0;
  pen.forEach((p, i) => {
    if (active(i) && !p.on) { p.on = true; p.value = Math.min(1, Math.max(0, signal[i](t))); }
    if (!active(i)) p.on = false;
    if (active(i) && sampleNow) p.value = Math.min(1, Math.max(0, signal[i](t)));
    const want = (p.value * 2 - 1) * D.LANE_HALF;
    for (let s = 0; s < 4; s++) { p.rate += ((want - p.y) * 140 - p.rate * 17) * dt / 4; p.y += p.rate * dt / 4; }
    if (calm) { p.y = want; p.rate = 0; }
    const zWant = active(i) ? D.PEN_DOWN : D.PEN_UP;
    p.z = calm ? zWant : p.z + (zWant - p.z) * (1 - Math.exp(-dt / 0.12));
    const list = marks[i];
    if (p.z < D.PEN_DOWN + 0.3) {
      const lastMark = list[list.length - 1];
      if (!lastMark || lastMark === null || travel - lastMark[2] > 0.6) list.push([D.PEN_X, LANES[i] + p.y, travel]);
    } else if (list.length && list[list.length - 1] !== null) list.push(null);
    while (list.length && (list[0] === null || list[0][0] + travel - list[0][2] > D.PAPER.x + D.PAPER.w + 2)) list.shift();
  });
  draw();
  const values = VIEWS[view].uses.map((i) => STREAMS[i] + " " + Math.round(pen[i].value * 100) + "%").join(" · ");
  say(VIEWS[view].name + " · " + values + " · " + (fast ? "500 ms" : "1 s"));
  frame = visible ? requestAnimationFrame(tick) : 0;
}
const run = () => { if (!frame && visible) { last = 0; frame = requestAnimationFrame(tick); } };
new IntersectionObserver((entries) => { visible = entries[entries.length - 1].isIntersecting; if (visible) run(); }, { rootMargin: "120px 0px" }).observe(stage);
const steer = () => { touring = false; describe(); clearTimeout(idleTimer); idleTimer = setTimeout(() => { touring = !still.matches; }, 3600); run(); };
stage.addEventListener("keydown", (event) => {
  if (event.key === "ArrowRight" || event.key === "ArrowUp") view = (view + 1) % 4;
  else if (event.key === "ArrowLeft" || event.key === "ArrowDown") view = (view + 3) % 4;
  else if (event.key >= "1" && event.key <= "4") view = Number(event.key) - 1;
  else if (event.key === "h" || event.key === "H" || event.key === " ") fast = !fast;
  else return;
  event.preventDefault();
  steer();
});
stage.addEventListener("click", (event) => {
  const box = svg.getBoundingClientRect();
  if ((event.clientX - box.left) / box.width > 0.5) { view = (view + 1) % 4; } else { fast = !fast; }
  steer();
});
describe();
draw();
`;

emit({
  name: "sentinel-recorder",
  title: "Sampler",
  svg,
  plate: {
    fig: "Fig 1",
    title: "The sampler",
    hint: "Views change on their own",
    readout: "CPU and memory · 1 s",
    aria: 'role="group" aria-label="Strip-chart recorder: left and right arrows change the view on screen, H switches between 1 second and 500 millisecond sampling"',
    keys: [
      { mark: "lit", label: "Pens: one per stream, lifted when the view doesn't need it" },
      { mark: "raised", label: "Paper: six units of travel for every sample" },
    ],
    caption:
      "Sentinel reads live data straight from your operating system, but only the streams the screen in front of you needs. Turn the knob to another view and the other pens lift: their streams pause until you look again. The signals drawn here are simulated.",
  },
  live,
  data: { P, LANES, VIEWS, STREAMS, LANE_HALF, PEN_X, PEN_DOWN, PEN_UP, PAPER, PAPER_Z, KNOB, KNOB_R, BOX_TOP, TOGGLE },
  css: `
.fig-trace{stroke-width:1.15}
.fig-trace[data-stream="0"]{stroke:var(--fig-s0,#74d3ae)}
.fig-trace[data-stream="1"]{stroke:var(--fig-s1,#7aa7ff)}
.fig-trace[data-stream="2"]{stroke:var(--fig-s2,#e3a857)}
.fig-trace[data-stream="3"]{stroke:var(--fig-s3,#e26ab0)}
`,
});
