/* Sampler: figure script, built by figures/sentinel-recorder.mjs */
(function(){
"use strict";
const k = window.isoKit;
const D = {"P":{"origin":[337.166944064625,167.27379077229523],"scale":1.818931222381684,"azimuth":34,"elevation":30},"LANES":[-31.5,-10.5,10.5,31.5],"VIEWS":[{"name":"Processes","uses":[0,1]},{"name":"Network","uses":[2]},{"name":"CPU and memory","uses":[0,1]},{"name":"Storage","uses":[3]}],"STREAMS":["CPU","memory","network","disk"],"LANE_HALF":8.5,"PEN_X":4,"PEN_DOWN":16.599999999999998,"PEN_UP":20.9,"PAPER":{"x":-72,"y":-42,"w":144,"d":84,"r":0.6},"PAPER_Z":16.4,"KNOB":[58,66],"KNOB_R":6.5,"BOX_TOP":14,"TOGGLE":[76,66]};
function mount(root){

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

}
function start(){document.querySelectorAll('[data-figure="sentinel-recorder"]').forEach(function(root){if(!root.dataset.live){root.dataset.live="1";mount(root);}});}
if(document.readyState==="loading")document.addEventListener("DOMContentLoaded",start);else start();
})();
