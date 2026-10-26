// Builds the static window markup; content that changes is filled in below.
(() => {
'use strict';
const h = (tag, attrs = {}, kids = []) => {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === 'html') e.innerHTML = v; else if (k === 'text') e.textContent = v; else e.setAttribute(k, v);
  }
  for (const kid of kids) e.append(kid);
  return e;
};
const chevron = '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6"><path d="M4 6l4 4 4-4"></path></svg>';
const square = '<svg viewBox="0 0 16 16"><rect x="4" y="4" width="8" height="8" rx="1" fill="currentColor"></rect></svg>';
const app = h('div', {class: 'app', id: 'app'}, [
  h('header', {class: 'title'}, [
    h('button', {class: 'btn appbtn', id: 'appBtn', 'aria-haspopup': 'menu', title: 'Application menu (F10)'}, [h('span', {class: 'logo'}), 'Wireroom']),
    h('div', {class: 'sep'}),
    h('div', {class: 'select'}, [
      h('button', {class: 'btn framed', id: 'dmdBtn', 'aria-haspopup': 'listbox', title: 'Demodulator'}, [h('span', {id: 'dmdName', text: 'Navtex'}), h('span', {html: chevron})]),
    ]),
    h('button', {class: 'btn framed', id: 'paramsBtn', title: 'Demodulator parameters'}, [h('span', {id: 'paramsSummary'})]),
    h('button', {class: 'btn framed chan', id: 'chanBtn', title: 'Channel: drag the band on the sonogram, or type'},
      [h('b', {id: 'chanF'}), ' Hz · ', h('span', {id: 'chanBw'}), ' Hz']),
    h('div', {class: 'sep'}),
    h('button', {class: 'btn icon primary', id: 'playBtn', title: 'Play (Space)'}),
    h('button', {class: 'btn icon', id: 'stopBtn', title: 'Stop and rewind', html: square}),
    h('div', {class: 'group', id: 'speeds', role: 'group', 'aria-label': 'Playback speed'}),
    h('div', {class: 'filename', id: 'filename', text: 'navtex-518k-sample.wav – Wireroom'}),
    h('div', {class: 'caps', 'aria-hidden': 'true'}, ['–', '□', '✕'].map(c => h('span', {text: c}))),
  ]),
  h('div', {class: 'main', id: 'main'}, [
    h('div', {class: 'plot', id: 'plot'}, [
      h('canvas', {id: 'plotCv'}),
      h('div', {class: 'zoom', id: 'zoomT', style: 'right:66px;bottom:8px'}, [
        h('button', {'data-z': 't-', title: 'Zoom time out (Ctrl+-)', text: '−'}),
        h('button', {'data-z': 't+', title: 'Zoom time in (Ctrl+=)', text: '+'}),
      ]),
      h('button', {class: 'btn follow', id: 'followBtn', 'aria-pressed': 'true', title: 'Keep the playhead in view (F)', text: 'Follow'}),
    ]),
    h('div', {class: 'vsplit', id: 'vsplit', role: 'separator', 'aria-orientation': 'vertical'}),
    h('div', {class: 'right'}, [
      h('section', {class: 'pane'}, [
        h('div', {class: 'pane-head', id: 'tabs'}),
        h('div', {class: 'list', id: 'results'}),
      ]),
      h('div', {class: 'hsplit', id: 'hsplit', role: 'separator', 'aria-orientation': 'horizontal'}),
      h('div', {class: 'bottom'}, [
        h('section', {class: 'pane'}, [
          h('div', {class: 'pane-head'}, [
            h('span', {class: 'ttl', text: 'Log'}), h('span', {class: 'count', id: 'logCount', text: '0'}),
            h('input', {class: 'filter', id: 'logFilter', placeholder: 'Filter', 'aria-label': 'Filter log'}),
          ]),
          h('div', {class: 'list log', id: 'log'}),
        ]),
        h('div', {class: 'vline'}),
        h('div', {class: 'const', id: 'const'}, [
          h('canvas', {id: 'constCv'}),
          h('div', {class: 'cap', id: 'constCap'}),
          h('div', {class: 'none', id: 'constNone', hidden: ''}),
        ]),
      ]),
    ]),
  ]),
  h('footer', {class: 'status'}, [
    h('span', {}, [h('i', {class: 'dot', id: 'stDot'}), h('b', {id: 'stState'})]),
    h('span', {class: 'prog'}, [h('i', {id: 'stProg'})]),
    h('span', {}, [h('b', {id: 'stPos', text: '00:00'}), ' / ', h('span', {id: 'stDur'})]),
    h('span', {class: 'hide-narrow'}, ['rate ', h('b', {id: 'stRate'})]),
    h('span', {class: 'hide-narrow', text: '12 kHz · real · f32'}),
    h('span', {class: 'r hide-narrow', id: 'stHover', text: '—'}),
    h('span', {}, ['backend ', h('b', {text: 'native'})]),
  ]),
]);
document.body.prepend(app);
})();

(() => {
'use strict';
const $ = id => document.getElementById(id);
const DUR = 860, FS = 12000, FMAX = FS / 2;
const ROWS = 1720, COLS = 480;              // 0.5 s rows, 12.5 Hz bins
const SPEEDS = [['1×',1],['2×',2],['4×',4],['8×',8],['16×',16],['Max',48]];

/* ---- demodulators as the core describes them ---- */
const DMDS = {
  Navtex: {
    about: 'Демодулятор сигналов NAVTEX (SITOR-B, F1B 100 Бод)',
    types: ['text','bits','ale','files'], points: 'fsk', freq: 1700, bw: 300,
    markers: [[-85,'info','F1'],[0,'service','Центральная частота'],[85,'info','F2']],
    params: [
      {k:'engine', l:'Тип обработчика', d:'Демодулятор и декодер SITOR-B', t:'enum', v:'fsk2-sitor', o:['codec2-sitor','fsk2-sitor','fldigi-sitor','fsk2-qdec'], devOnly:true},
      {k:'quality', l:'Качество знаков', d:'Уверенность декодера по двум копиям знака', t:'bool', v:false, devOnly:true},
      {k:'first', l:'Номер первого сообщения', d:'Номер в имени файла первого сообщения', t:'num', v:1},
      {k:'suffix', l:'Окончание имени', d:'Постоянная часть имени файла после номера', t:'str', v:''},
      {k:'bits', l:'Мягкие биты сообщения', d:'Файл .bits с мягкими битами знаков', t:'bool', v:false, devOnly:true},
      {k:'wav', l:'Отсчёты сообщения', d:'Файл .wav с отсчётами и запасом 5 с', t:'bool', v:false, devOnly:true},
    ],
    summary: p => `${p.engine} · from ${p.first}`,
  },
  Morse: {
    about: 'Демодулятор сигналов НС-1 (CW, Морзе)',
    types: ['text'], points: null, freq: 800, bw: 400,
    markers: [[0,'service','Центральная частота']],
    params: [
      {k:'engine', l:'Тип обработчика', d:'Тип приёмного тракта CW', t:'enum', v:'gate2-model', o:['fldigi4','ggmorse','multimon','xdemorse','own','libcw','morsepro','model','gate2','gate2-libcw','gate2-morsepro','gate2-model']},
      {k:'trace', l:'Трассировка событий', d:'Писать события тона в лог', t:'bool', v:false, devOnly:true},
    ],
    summary: p => p.engine,
  },
  Chn4x4: {
    about: 'Демодулятор сигналов Chinese 4+4 (22Ф15)',
    types: ['text','bits'], points: 'psk4', freq: 3300, bw: 1000,
    markers: [[-350,'info',''],[-250,'info',''],[-150,'info',''],[-50,'info',''],[0,'service','Центральная частота'],[50,'info',''],[150,'info',''],[250,'info',''],[350,'info','']],
    params: [],
    summary: () => 'defaults',
  },
};
const TYPE_LABEL = {text:'Text', bits:'Bits', ale:'ALE', files:'Files'};

/* ---- example events in the sample recording (seconds) ---- */
const NAVTEX_MSGS = [
  [38, 96, 'SA12', 'ZCZC SA12\n241130 UTC OCT 26\nNAVAREA I 412/26\nDOVER STRAIT. LIGHT BUOY 51-08.6N 001-46.1E UNLIT.\nNNNN'],
  [170, 214, 'SB37', 'ZCZC SB37\n241205 UTC OCT 26\nGALE WARNING. SOLE, FASTNET.\nSOUTHWEST 8 BACKING SOUTH 9 LATER.\nNNNN'],
  [285, 352, 'SA13', 'ZCZC SA13\n241240 UTC OCT 26\nNAVAREA I 413/26\nNORTH SEA. SURVEY VESSEL OPERATING\n53-21N 003-12E. WIDE BERTH REQUESTED.\nNNNN'],
  [430, 470, 'SE04', 'ZCZC SE04\n241318 UTC OCT 26\nNO MESSAGES ON HAND.\nNNNN'],
  [555, 640, 'SB38', 'ZCZC SB38\n241401 UTC OCT 26\nGALE WARNING CANCELLED. PLYMOUTH.\nNNNN'],
  [720, 790, 'SA14', 'ZCZC SA14\n241450 UTC OCT 26\nNAVAREA I 414/26\nTHAMES ESTUARY. WRECK 51-33.2N 001-21.9E\nMARKED BY EAST CARDINAL BUOY.\nNNNN'],
];
const CW_BURSTS = [[20,70],[140,190],[260,300],[380,450],[520,560],[650,700],[760,830]];
const CW_TEXT = ['VVV VVV DE TEST1 QSA? K', 'TEST1 DE TEST2 R QSA 4 QRV K', 'NR 017 GR 12 = 47215 03981 =', 'QRU QRT 73 SK', 'CQ CQ DE TEST1 K', 'TEST1 DE TEST3 GA', 'R TU 73 E E'];
const C44_BURSTS = [[60,180],[330,420],[600,700]];
const groups = n => Array.from({length:n}, (_,i) => String((i*7919+31337)%100000).padStart(5,'0')).join(' ');
const bitsOf = (seed, n) => { let s = seed, r = ''; for (let i = 0; i < n; i++) { s = (s*1103515245+12345) & 0x7fffffff; r += (s>>16)&1; if (i%8===7) r += ' '; } return r.trim(); };

function scriptFor(name) {
  const ev = [];
  if (name === 'Navtex') NAVTEX_MSGS.forEach(([a,b,id,text],i) => {
    ev.push({t:a, log:`phasing detected, Δf ${(Math.sin(i)*6).toFixed(1)} Hz`});
    ev.push({t:a+6, log:`sync: SITOR-B FEC, header ${id}`});
    ev.push({t:a+8, kind:'text', start:a, end:b, id, text, live:true});
    ev.push({t:a+10, kind:'ale', start:a, end:b, text:`${id.slice(0,1)} ${id.slice(1,2)} ${id.slice(2)}  station ${id[0]} · subject ${id[1]} · serial ${id.slice(2)}`});
    ev.push({t:b, kind:'bits', start:a, end:b, text:bitsOf(i+3, 96)});
    ev.push({t:b, kind:'files', start:a, end:b, name:`${String(i+1).padStart(4,'0')}.txt`, mime:'text/plain', size:text.length});
    ev.push({t:b, log:`message ${id} closed, ${Math.round(2+i%3)} errors corrected`});
  });
  if (name === 'Morse') CW_BURSTS.forEach(([a,b],i) => {
    ev.push({t:a+4, kind:'text', start:a, end:b, id:`CW ${String(i+1).padStart(2,'0')}`, text:CW_TEXT[i], live:true});
  });
  if (name === 'Chn4x4') C44_BURSTS.forEach(([a,b],i) => {
    ev.push({t:a, log:'tone grid locked, 8 of 8 tones'});
    ev.push({t:a+5, kind:'text', start:a, end:b, id:`Group set ${i+1}`, text:groups(12+i*4), live:true});
    ev.push({t:b, kind:'bits', start:a, end:b, text:bitsOf(i+11, 128)});
    ev.push({t:b, log:`burst ${i+1} ended, ${b-a} s`});
  });
  return ev.sort((x,y) => x.t - y.t);
}

/* ---- synthetic sonogram of the sample recording ---- */
const db = new Float32Array(ROWS * COLS);
(function synth() {
  let s = 12345; const rnd = () => (s = (s*16807) % 2147483647) / 2147483647;
  const bin = f => Math.round(f / FMAX * COLS);
  const add = (r, f, p, w=1) => { const c = bin(f); for (let k = -w; k <= w; k++) { const j = c+k; if (j>=0 && j<COLS) { const i = r*COLS+j; db[i] = Math.max(db[i], p - Math.abs(k)*9); } } };
  for (let r = 0; r < ROWS; r++) {
    const t = r * DUR / ROWS;
    for (let c = 0; c < COLS; c++) {
      const f = c / COLS * FMAX;
      const shape = -8 * Math.pow(f/FMAX - 0.35, 2) - (f < 150 ? 10 : 0) - (f > 5400 ? 14 : 0);
      db[r*COLS+c] = -96 + shape + rnd()*9;
    }
    if (Math.sin(t/37)+rnd() > .2) add(r, 3000, -58 + Math.sin(t/11)*4, 0);
    if (NAVTEX_MSGS.some(([a,b]) => t>=a && t<=b)) { add(r, 1615, -40 + rnd()*4); add(r, 1785, -40 + rnd()*4); }
    if (CW_BURSTS.some(([a,b]) => t>=a && t<=b) && rnd() > .35) add(r, 800, -44 + rnd()*5, 0);
    if (C44_BURSTS.some(([a,b]) => t>=a && t<=b)) for (let k = -350; k <= 350; k += 100) if (k || 1) if (rnd() > .45) add(r, 3300+k, -48 + rnd()*5, 0);
    if ((t % 140) > 100 && (t % 140) < 108) for (let f = 4300; f < 4700; f += 12.5) add(r, f, -62 + rnd()*6, 0);
  }
})();
// perceptual dark-to-bright ramp used by the waterfall in both themes
const LUT = (() => {
  const stops = [[0,[8,8,14]],[.25,[38,22,84]],[.5,[128,34,104]],[.72,[220,72,62]],[.88,[248,160,48]],[1,[252,248,200]]];
  const out = new Uint8ClampedArray(256*3);
  for (let i = 0; i < 256; i++) { const x = i/255; let k = 0; while (x > stops[k+1][0]) k++;
    const [a,ca] = stops[k], [b,cb] = stops[k+1], u = (x-a)/(b-a);
    for (let j = 0; j < 3; j++) out[i*3+j] = ca[j] + (cb[j]-ca[j])*u; }
  return out;
})();
const off = document.createElement('canvas'); off.width = COLS; off.height = ROWS;
(() => { const g = off.getContext('2d'), img = g.createImageData(COLS, ROWS);
  for (let i = 0; i < ROWS*COLS; i++) { const v = Math.max(0, Math.min(255, (db[i] + 100) / 64 * 255)) | 0;
    img.data[i*4] = LUT[v*3]; img.data[i*4+1] = LUT[v*3+1]; img.data[i*4+2] = LUT[v*3+2]; img.data[i*4+3] = 255; }
  g.putImageData(img, 0, 0); })();

/* ---- state ---- */
const st = {
  dmd: 'Navtex', params: {}, devParams: false,
  playing: true, speed: 8, pos: 236, follow: true,
  viewStart: 120, viewSpan: 150,
  results: [], logs: [], script: [], next: 0,
  tab: 'text', sel: null, hover: null, drag: null,
  pts: [],
};
const fmt = t => { t = Math.max(0, t); const m = Math.floor(t/60), s = Math.floor(t%60); return `${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}`; };
const fmtMs = t => `${fmt(t)}.${Math.floor((t%1)*10)}`;
const fmtHz = f => { const [i, d] = f.toFixed(1).split('.'); return `${i.replace(new RegExp('\\B(?=(\\d{3})+(?!\\d))', 'g'), ' ')}.${d}`; };

function setDmd(name, keepPos) {
  st.dmd = name; const d = DMDS[name];
  st.params = Object.fromEntries(d.params.map(p => [p.k, p.v]));
  st.freq = d.freq; st.bw = d.bw;
  st.script = scriptFor(name); st.next = 0; st.results = []; st.logs = []; st.sel = null;
  if (!keepPos) st.pos = 0;
  st.tab = d.types[0];
  pushLog(0, `${name}: ${d.about}`); pushLog(0, `channel ${fmtHz(st.freq)} Hz, bandwidth ${st.bw} Hz`);
  advanceTo(st.pos);
  $('dmdName').textContent = name;
  $('constNone').hidden = !!d.points; $('constNone').textContent = `${name} emits no phase points`;
  renderChrome(); renderTabs(); renderResults(); renderLog();
}
function pushLog(t, text) { st.logs.push({t, text}); }
function advanceTo(t) {
  let changed = false;
  while (st.next < st.script.length && st.script[st.next].t <= t) {
    const e = st.script[st.next++]; changed = true;
    if (e.log) pushLog(e.t, e.log);
    if (e.kind) { st.results.push({...e}); if (e.kind === 'text') pushLog(e.t, `${e.id}: text started`); }
  }
  st.results.forEach(r => { if (r.live && t >= r.end) { r.live = false; changed = true; } });
  return changed;
}

/* ---- toolbar ---- */
const ICON_PLAY = '<svg viewBox="0 0 16 16"><path d="M5 3.5v9l7.5-4.5z" fill="currentColor"/></svg>';
const ICON_PAUSE = '<svg viewBox="0 0 16 16"><rect x="4" y="3.5" width="3" height="9" rx=".8" fill="currentColor"/><rect x="9" y="3.5" width="3" height="9" rx=".8" fill="currentColor"/></svg>';
function renderChrome() {
  $('playBtn').innerHTML = st.playing ? ICON_PAUSE : ICON_PLAY;
  $('playBtn').title = st.playing ? 'Pause (Space)' : 'Play (Space)';
  $('paramsSummary').textContent = DMDS[st.dmd].summary(st.params);
  $('chanF').textContent = fmtHz(st.freq); $('chanBw').textContent = st.bw;
  $('speeds').innerHTML = SPEEDS.map(([l,v]) => `<button aria-pressed="${st.speed===v}" data-s="${v}" title="${v===48?'As fast as the core allows':l+' realtime'}">${l}</button>`).join('');
  $('stDot').className = 'dot ' + (st.playing ? 'run' : 'pause');
  $('stState').textContent = st.pos >= DUR ? 'Finished' : st.playing ? 'Playing' : (st.pos ? 'Paused' : 'Stopped');
  $('stRate').textContent = st.speed === 48 ? `×${(40 + Math.random()*9).toFixed(1)} (max)` : `×${st.speed}`;
}
$('speeds').addEventListener('click', e => { const b = e.target.closest('button'); if (!b) return; st.speed = +b.dataset.s; renderChrome(); });
$('playBtn').onclick = () => { if (st.pos >= DUR) restart(); st.playing = !st.playing; renderChrome(); };
$('stopBtn').onclick = () => { st.playing = false; restart(); renderChrome(); };
function restart() { setDmd(st.dmd, false); st.viewStart = 0; }
$('followBtn').onclick = () => { st.follow = !st.follow; $('followBtn').setAttribute('aria-pressed', st.follow); };

/* ---- popovers ---- */
let pop = null;
function closePop() { if (pop) { pop.remove(); pop = null; } }
function openPop(anchor, html, cls='') {
  closePop(); pop = document.createElement('div'); pop.className = 'pop ' + cls; pop.innerHTML = html;
  pop.style.left = (anchor.offsetLeft) + 'px'; anchor.closest('.title').appendChild(pop); return pop;
}
document.addEventListener('pointerdown', e => { if (pop && !pop.contains(e.target) && !e.target.closest('#appBtn,#dmdBtn,#paramsBtn,#chanBtn')) closePop(); });
document.addEventListener('keydown', e => {
  if (e.key === 'Escape' && pop) { closePop(); return; }
  if (e.target.matches('input,select')) return;
  if (e.code === 'Space') { e.preventDefault(); $('playBtn').click(); }
  if (e.key === 'f' || e.key === 'F') $('followBtn').click();
});
$('appBtn').onclick = () => { if (pop?.dataset.k === 'app') return closePop(); const p = openPop($('appBtn'), `
  <div class="menu-row">Open recording… <span class="kbd">Ctrl+O</span></div>
  <div class="menu-row">navtex-518k-sample.wav <span class="kbd">1</span></div>
  <div class="menu-row">cw-net-0412.wav <span class="kbd">2</span></div>
  <div class="menu-sep"></div>
  <div class="menu-row">Open output folder <span class="kbd">Ctrl+Shift+O</span></div>
  <div class="menu-row">Backend <small>native · replay</small></div>
  <div class="menu-sep"></div>
  <div class="menu-row">View <small>›</small></div>
  <div class="menu-row">Settings <span class="kbd">Ctrl+,</span></div>`); p.dataset.k = 'app'; };
$('dmdBtn').onclick = () => { if (pop?.dataset.k === 'dmd') return closePop();
  const p = openPop($('dmdBtn'), Object.entries(DMDS).map(([n,d]) => `<div class="menu-row" role="option" data-n="${n}" aria-selected="${n===st.dmd}"><span><b>${n}</b><br><small>${d.about}</small></span></div>`).join('') +
    `<div class="menu-sep"></div><div class="menu-row"><small>Changing the demodulator stops processing and clears results</small></div>`);
  p.dataset.k = 'dmd'; p.style.minWidth = '320px';
  p.addEventListener('click', e => { const r = e.target.closest('[data-n]'); if (!r) return; closePop(); st.playing = false; setDmd(r.dataset.n, false); st.viewStart = 0; renderChrome(); });
};
function paramsHtml() {
  const d = DMDS[st.dmd];
  const field = p => {
    const v = st.params[p.k]; let input;
    if (p.t === 'enum') input = `<select id="p-${p.k}" data-k="${p.k}">${p.o.map(o => `<option value="${o}"${o===v?' selected':''}>${o}</option>`).join('')}</select>`;
    else if (p.t === 'bool') input = `<input type="checkbox" id="p-${p.k}" data-k="${p.k}"${v?' checked':''}>`;
    else input = `<input type="${p.t==='num'?'number':'text'}" id="p-${p.k}" data-k="${p.k}" value="${v}">`;
    return `<div class="field"><label for="p-${p.k}">${p.l}<small>${p.d}</small></label>${input}</div>`;
  };
  const main = d.params.filter(p => !p.devOnly), dev = d.params.filter(p => p.devOnly);
  return `<h3>${st.dmd} parameters</h3><div class="info">${d.about}</div>
    ${main.length ? main.map(field).join('') : '<div class="info">This demodulator has no user parameters.</div>'}
    ${dev.length ? `<label class="toggle"><input type="checkbox" id="devToggle"${st.devParams?' checked':''}> Developer parameters (${dev.length})</label>
      <div class="dev"${st.devParams?'':' hidden'}>${dev.map(field).join('')}</div>` : ''}
    <div class="params-foot"><span class="info" style="margin:0">Applied on the next start</span><button class="btn framed" id="pReset">Reset to defaults</button></div>`;
}
$('paramsBtn').onclick = () => { if (pop?.dataset.k === 'par') return closePop();
  const p = openPop($('paramsBtn'), paramsHtml(), 'params'); p.dataset.k = 'par';
  p.addEventListener('change', e => {
    if (e.target.id === 'devToggle') { st.devParams = e.target.checked; p.querySelector('.dev').hidden = !st.devParams; return; }
    const k = e.target.dataset.k; if (!k) return;
    st.params[k] = e.target.type === 'checkbox' ? e.target.checked : e.target.type === 'number' ? +e.target.value : e.target.value;
    renderChrome();
  });
  p.addEventListener('click', e => { if (e.target.id !== 'pReset') return;
    st.params = Object.fromEntries(DMDS[st.dmd].params.map(q => [q.k, q.v])); p.innerHTML = paramsHtml(); renderChrome(); });
};
$('chanBtn').onclick = () => { if (pop?.dataset.k === 'ch') return closePop();
  const p = openPop($('chanBtn'), `<div class="params" style="width:260px;padding:6px">
    <div class="field"><label for="chF">Centre<small>Hz, 0 … ${FMAX}</small></label><input type="number" id="chF" step="0.1" value="${st.freq.toFixed(1)}"></div>
    <div class="field"><label>Bandwidth<small>from the demodulator mask</small></label><span class="chan">${st.bw} Hz</span></div>
    <div class="info" style="margin:0">${st.playing ? 'Retuning while playing restarts the channel at the playhead.' : 'Or drag the band on the sonogram.'}</div></div>`);
  p.dataset.k = 'ch';
  p.querySelector('#chF').addEventListener('change', e => { setFreq(+e.target.value); });
};
function setFreq(f) {
  f = Math.max(st.bw/2, Math.min(FMAX - st.bw/2, f));
  if (Math.abs(f - st.freq) < .05) return;
  st.freq = f; renderChrome();
}

/* ---- results panel ---- */
function renderTabs() {
  const d = DMDS[st.dmd];
  $('tabs').innerHTML = '<span class="ttl">Results</span>' + d.types.map(t => {
    const n = st.results.filter(r => r.kind === t).length;
    return `<button class="tab" role="tab" data-t="${t}" aria-selected="${st.tab===t}">${TYPE_LABEL[t]}<span class="count">${n}</span></button>`;
  }).join('');
}
$('tabs').addEventListener('click', e => { const b = e.target.closest('[data-t]'); if (!b) return; st.tab = b.dataset.t; renderTabs(); renderResults(); });
function renderResults() {
  const el = $('results'), items = st.results.filter(r => r.kind === st.tab);
  if (!items.length) { el.innerHTML = `<div class="empty">No ${TYPE_LABEL[st.tab].toLowerCase()} results yet. They appear here as playback reaches them.</div>`; return; }
  const span = r => `${fmt(r.start)} – ${r.live ? '…' : fmt(r.end)}`;
  const idx = r => st.results.indexOf(r);
  if (st.tab === 'files') {
    const rows = items.map(r => `<tr data-i="${idx(r)}"${st.sel===idx(r)?' class="sel"':''}><td>${r.name}</td><td>${r.mime}</td><td class="num">${r.size} B</td><td class="num">${fmt(r.start)}</td></tr>`).join('');
    el.innerHTML = `<table class="files"><thead><tr><th>File</th><th>Type</th><th style="text-align:right">Size</th><th>At</th></tr></thead><tbody>${rows}</tbody></table>`;
    return;
  }
  el.innerHTML = items.slice().reverse().map(r => {
    const i = idx(r), state = r.live ? '<span class="pill live">receiving</span>' : (st.tab === 'text' ? '<span class="pill ok">complete</span>' : '');
    const head = r.id ? `<b>${r.id}</b>` : `<b>${TYPE_LABEL[r.kind]}</b>`;
    let body = r.text;
    if (r.live && r.kind === 'text') body = body.slice(0, Math.max(4, Math.floor(body.length * Math.min(1, (st.pos - r.start) / (r.end - r.start)))));
    return `<div class="msg${st.sel===i?' sel':''}${r.live?' live':''}" data-i="${i}"><div class="msg-h"><span>${head} · ${span(r)}</span>${state}</div><pre>${body.replaceAll('<', '&lt;')}</pre></div>`;
  }).join('');
}
$('results').addEventListener('click', e => { const r = e.target.closest('[data-i]'); if (!r) return; select(+r.dataset.i, true); });
$('results').addEventListener('pointerover', e => { const r = e.target.closest('[data-i]'); st.hover = r ? +r.dataset.i : null; });
$('results').addEventListener('pointerleave', () => { st.hover = null; });
function select(i, reveal) {
  st.sel = i; const r = st.results[i];
  if (reveal && r) { st.follow = false; $('followBtn').setAttribute('aria-pressed', false);
    st.viewStart = Math.max(0, Math.min(DUR - st.viewSpan, (r.start + r.end) / 2 - st.viewSpan / 2)); }
  renderResults();
}

/* ---- log ---- */
function renderLog() {
  const q = $('logFilter').value.trim().toLowerCase();
  const rows = st.logs.filter(l => !q || l.text.toLowerCase().includes(q));
  const el = $('log'), atEnd = el.scrollTop + el.clientHeight >= el.scrollHeight - 4;
  el.innerHTML = rows.map(l => `<div title="${l.text}"><time>${fmtMs(l.t)}</time>${l.text}</div>`).join('');
  $('logCount').textContent = st.logs.length;
  if (atEnd) el.scrollTop = el.scrollHeight;
}
$('logFilter').addEventListener('input', renderLog);

/* ---- plot ---- */
const cv = $('plotCv'), g = cv.getContext('2d');
const MINI = 42, TRUL = 58, FRUL = 24, SPEC = 120, GAP = 1;
let W = 0, H = 0, DPR = 1;
function geom() {
  const x0 = MINI + 1, x1 = W - TRUL, sy0 = 0, sy1 = SPEC, wy0 = SPEC + GAP, wy1 = H - FRUL;
  return {x0, x1, sy0, sy1, wy0, wy1, fx: f => x0 + f / FMAX * (x1 - x0), xf: x => (x - x0) / (x1 - x0) * FMAX,
    ty: t => wy0 + (t - st.viewStart) / st.viewSpan * (wy1 - wy0), yt: y => st.viewStart + (y - wy0) / (wy1 - wy0) * st.viewSpan};
}
function resize() {
  const r = cv.getBoundingClientRect(); DPR = devicePixelRatio || 1; W = r.width; H = r.height;
  cv.width = W * DPR; cv.height = H * DPR; resizeConst();
}
function tok(name) { return getComputedStyle(document.documentElement).getPropertyValue(name).trim(); }
function drawPlot() {
  if (!W) return; const G = geom();
  g.setTransform(DPR,0,0,DPR,0,0); g.clearRect(0,0,W,H);
  g.fillStyle = tok('--plot-bg'); g.fillRect(0,0,W,H);
  const ink = tok('--plot-ink'), grid = tok('--plot-grid'), chc = tok('--channel'), svc = tok('--service');
  g.font = `11px ${tok('--font-mono')}`; g.textBaseline = 'middle';

  // minimap: whole recording, visible window, processed share
  g.imageSmoothingEnabled = true;
  g.drawImage(off, 0, 0, COLS, ROWS, 0, 0, MINI, H);
  g.fillStyle = 'rgba(0,0,0,.45)'; const py = st.pos / DUR * H; g.fillRect(0, py, MINI, H - py);
  g.strokeStyle = '#fff'; g.lineWidth = 1; g.strokeRect(.5, st.viewStart / DUR * H + .5, MINI - 1, st.viewSpan / DUR * H - 1);
  g.fillStyle = chc; g.fillRect(0, py - 1, MINI, 2);
  g.fillStyle = 'rgba(255,255,255,.18)'; g.fillRect(MINI, 0, 1, H);

  // sonogram slice
  const r0 = st.viewStart / DUR * ROWS, rn = st.viewSpan / DUR * ROWS;
  g.drawImage(off, 0, r0, COLS, rn, G.x0, G.wy0, G.x1 - G.x0, G.wy1 - G.wy0);
  const yp = G.ty(st.pos);
  if (yp < G.wy1) { g.fillStyle = 'rgba(0,0,0,.5)'; g.fillRect(G.x0, Math.max(G.wy0, yp), G.x1 - G.x0, G.wy1 - Math.max(G.wy0, yp)); }

  // grid
  g.strokeStyle = grid; g.beginPath();
  for (let f = 500; f < FMAX; f += 500) { const x = Math.round(G.fx(f)) + .5; g.moveTo(x, G.sy0); g.lineTo(x, G.wy1); }
  const tstep = st.viewSpan > 300 ? 60 : st.viewSpan > 120 ? 30 : st.viewSpan > 50 ? 10 : 5;
  for (let t = Math.ceil(st.viewStart / tstep) * tstep; t < st.viewStart + st.viewSpan; t += tstep) { const y = Math.round(G.ty(t)) + .5; g.moveTo(G.x0, y); g.lineTo(G.x1, y); }
  g.stroke();

  // result spans on the sonogram
  const d = DMDS[st.dmd];
  st.results.forEach((r, i) => {
    if (r.kind !== d.types[0]) return;
    const ya = G.ty(r.start), yb = G.ty(r.live ? st.pos : r.end); if (yb < G.wy0 || ya > G.wy1) return;
    const xa = G.fx(st.freq - st.bw/2) - 6, xb = G.fx(st.freq + st.bw/2) + 6;
    const on = i === st.sel || i === st.hover;
    g.strokeStyle = on ? '#fff' : 'rgba(255,255,255,.55)'; g.lineWidth = on ? 2 : 1; g.setLineDash(r.live ? [4,3] : []);
    const top = Math.max(G.wy0, ya), bot = Math.min(G.wy1, yb);
    g.strokeRect(xa + .5, top + .5, xb - xa, bot - top); g.setLineDash([]);
    if (top < G.wy1 - 14) { const label = r.id || TYPE_LABEL[r.kind]; const tw = g.measureText(label).width + 10;
      g.fillStyle = on ? '#fff' : 'rgba(255,255,255,.75)'; g.fillRect(xb + 2, top, tw, 16);
      g.fillStyle = '#111'; g.fillText(label, xb + 7, top + 8); }
  });

  // spectrum at the playhead
  g.fillStyle = '#121219'; g.fillRect(G.x0, G.sy0, G.x1 - G.x0, G.sy1 - G.sy0);
  const row = Math.min(ROWS - 1, Math.floor(st.pos / DUR * ROWS));
  const avg = new Float32Array(COLS);
  for (let k = 0; k < 4; k++) { const rr = Math.max(0, row - k); for (let c = 0; c < COLS; c++) avg[c] += db[rr*COLS+c] / 4; }
  const sy = v => G.sy1 - 6 - (v + 100) / 70 * (G.sy1 - G.sy0 - 12);
  g.beginPath(); g.moveTo(G.x0, G.sy1);
  for (let c = 0; c < COLS; c++) g.lineTo(G.fx(c / COLS * FMAX), Math.max(G.sy0 + 2, sy(avg[c])));
  g.lineTo(G.x1, G.sy1); g.closePath();
  g.fillStyle = 'rgba(120,150,255,.22)'; g.fill(); g.strokeStyle = 'rgba(170,190,255,.9)'; g.lineWidth = 1; g.stroke();
  g.strokeStyle = grid; g.beginPath(); for (let v = -90; v <= -40; v += 10) { const y = Math.round(sy(v)) + .5; g.moveTo(G.x0, y); g.lineTo(G.x1, y); } g.stroke();
  g.fillStyle = ink; g.textAlign = 'left';
  for (let v = -90; v <= -40; v += 20) g.fillText(`${v}`, G.x1 + 6, sy(v));
  g.fillText('dBFS', G.x1 + 6, G.sy0 + 10);

  // channel band and mask markers, through spectrum and sonogram
  const ba = G.fx(st.freq - st.bw/2), bb = G.fx(st.freq + st.bw/2);
  g.fillStyle = 'rgba(243,179,61,.10)'; g.fillRect(ba, G.sy0, bb - ba, G.wy1 - G.sy0);
  g.strokeStyle = chc; g.lineWidth = 1.5; g.beginPath(); g.moveTo(ba, G.sy0); g.lineTo(ba, G.wy1); g.moveTo(bb, G.sy0); g.lineTo(bb, G.wy1); g.stroke();
  d.markers.forEach(([df, type, title]) => {
    const x = Math.round(G.fx(st.freq + df)) + .5;
    g.strokeStyle = type === 'service' ? svc : 'rgba(255,255,255,.65)'; g.lineWidth = 1; g.setLineDash(type === 'service' ? [] : [3,3]);
    g.beginPath(); g.moveTo(x, G.sy0 + 14); g.lineTo(x, G.wy1); g.stroke(); g.setLineDash([]);
    if (title && type === 'info') { g.fillStyle = '#fff'; g.textAlign = 'center'; g.fillText(title, x, G.sy0 + 8); }
  });
  // drag handle tab
  g.fillStyle = chc; g.fillRect(ba, G.sy1 - 3, bb - ba, 3);

  // playhead
  if (yp >= G.wy0 && yp <= G.wy1) {
    g.fillStyle = chc; g.fillRect(G.x0, yp - 1, G.x1 - G.x0, 2);
    const label = fmtMs(st.pos); g.font = `500 11px ${tok('--font-mono')}`; const tw = g.measureText(label).width + 10;
    g.fillRect(G.x1, yp - 9, Math.min(TRUL, tw), 18); g.fillStyle = '#111'; g.textAlign = 'left'; g.fillText(label, G.x1 + 5, yp);
    g.font = `11px ${tok('--font-mono')}`;
  }

  // rulers
  g.fillStyle = ink; g.textAlign = 'center';
  for (let f = 0; f <= FMAX; f += 1000) g.fillText(f === 0 ? '0' : `${f/1000}k`, Math.min(G.x1 - 10, Math.max(G.x0 + 8, G.fx(f))), H - FRUL / 2);
  g.textAlign = 'right'; g.fillText('Hz', W - 6, H - FRUL / 2);
  g.textAlign = 'left';
  for (let t = Math.ceil(st.viewStart / tstep) * tstep; t < st.viewStart + st.viewSpan; t += tstep) {
    const y = G.ty(t); if (Math.abs(y - yp) > 12 && y > G.wy0 + 6) g.fillText(fmt(t), G.x1 + 6, y);
  }
  g.fillText('mm:ss', G.x1 + 6, G.wy0 + 8);
  g.fillStyle = 'rgba(255,255,255,.18)'; g.fillRect(G.x0, G.sy1, G.x1 - G.x0, GAP);
}

// gestures: drag the band, drag the minimap, wheel to scroll, hover readout
cv.addEventListener('pointerdown', e => {
  const G = geom(), x = e.offsetX, y = e.offsetY;
  if (x < MINI) { st.drag = {k:'mini'}; jumpMini(y); }
  else if (x > G.x0 && x < G.x1 && y < G.wy1 && Math.abs(G.xf(x) - st.freq) < st.bw / 2 + 40) st.drag = {k:'band', dx: G.xf(x) - st.freq};
  else if (x > G.x0 && x < G.x1 && y > G.wy0 && y < G.wy1) st.drag = {k:'pan', y, v: st.viewStart};
  if (st.drag) { cv.setPointerCapture(e.pointerId); if (st.drag.k !== 'band') { st.follow = false; $('followBtn').setAttribute('aria-pressed', false); } }
});
cv.addEventListener('pointermove', e => {
  const G = geom(), x = e.offsetX, y = e.offsetY;
  const inBand = x > G.x0 && x < G.x1 && y < G.wy1 && Math.abs(G.xf(x) - st.freq) < st.bw / 2 + 40;
  cv.style.cursor = st.drag?.k === 'band' || inBand ? 'ew-resize' : x < MINI ? 'pointer' : st.drag?.k === 'pan' ? 'grabbing' : 'crosshair';
  if (x > G.x0 && x < G.x1) $('stHover').textContent = y > G.wy0 && y < G.wy1 ? `${fmtHz(G.xf(x))} Hz · ${fmtMs(G.yt(y))}` : `${fmtHz(G.xf(x))} Hz`;
  if (!st.drag) return;
  if (st.drag.k === 'band') setFreq(Math.round((G.xf(x) - st.drag.dx) * 10) / 10);
  if (st.drag.k === 'mini') jumpMini(y);
  if (st.drag.k === 'pan') st.viewStart = clampView(st.drag.v - (y - st.drag.y) / (G.wy1 - G.wy0) * st.viewSpan);
});
cv.addEventListener('pointerup', () => { if (st.drag?.k === 'band') pushLog(st.pos, `channel retuned to ${fmtHz(st.freq)} Hz`), renderLog(); st.drag = null; });
cv.addEventListener('pointerleave', () => { $('stHover').textContent = '—'; });
cv.addEventListener('wheel', e => { e.preventDefault(); st.follow = false; $('followBtn').setAttribute('aria-pressed', false);
  if (e.ctrlKey) zoom(e.deltaY > 0 ? 1.25 : 0.8); else st.viewStart = clampView(st.viewStart + e.deltaY / 400 * st.viewSpan); }, {passive:false});
const clampView = v => Math.max(0, Math.min(DUR - st.viewSpan, v));
function jumpMini(y) { st.viewStart = clampView(y / H * DUR - st.viewSpan / 2); }
function zoom(k) { const c = st.viewStart + st.viewSpan / 2; st.viewSpan = Math.max(20, Math.min(DUR, st.viewSpan * k)); st.viewStart = clampView(c - st.viewSpan / 2); }
document.querySelectorAll('[data-z]').forEach(b => b.onclick = () => zoom(b.dataset.z === 't+' ? 0.7 : 1.4));

/* ---- constellation: points at the playhead ---- */
const cc = $('constCv'), cg = cc.getContext('2d'); let CW = 0;
function resizeConst() { const r = cc.getBoundingClientRect(); CW = Math.min(r.width, r.height); cc.width = r.width * DPR; cc.height = r.height * DPR; cg.setTransform(DPR,0,0,DPR,0,0); }
function gauss() { return (Math.random() + Math.random() + Math.random() - 1.5) * 0.8; }
function inBurst() {
  const t = st.pos, n = st.dmd;
  if (n === 'Navtex') return NAVTEX_MSGS.some(([a,b]) => t>=a && t<=b);
  if (n === 'Chn4x4') return C44_BURSTS.some(([a,b]) => t>=a && t<=b);
  return false;
}
function drawConst(dt) {
  const d = DMDS[st.dmd]; const r = cc.getBoundingClientRect(); if (!r.width) return;
  const w = r.width, h = r.height, s = Math.min(w, h) / 2 - 14, cx = w / 2, cy = h / 2 + 6;
  cg.fillStyle = tok('--plot-bg'); cg.fillRect(0, 0, w, h);
  cg.strokeStyle = tok('--plot-grid'); cg.lineWidth = 1; cg.beginPath();
  cg.moveTo(cx - s, cy + .5); cg.lineTo(cx + s, cy + .5); cg.moveTo(cx + .5, cy - s); cg.lineTo(cx + .5, cy + s); cg.stroke();
  cg.beginPath(); cg.arc(cx, cy, s, 0, Math.PI * 2); cg.stroke();
  if (!d.points) return;
  if (st.playing && st.pos < DUR) {
    const n = Math.min(60, Math.ceil(dt * 0.06 * Math.min(st.speed, 16) * 6)), burst = inBurst(), noise = burst ? 0.09 : 0.55;
    for (let i = 0; i < n; i++) {
      let x, y;
      if (d.points === 'fsk') { const sym = Math.random() < .5 ? -1 : 1; x = sym * 0.72 + gauss() * noise; y = gauss() * noise * 0.8; }
      else { const a = (Math.floor(Math.random() * 4) + .5) * Math.PI / 2; x = Math.cos(a) * .7 + gauss() * noise; y = Math.sin(a) * .7 + gauss() * noise; }
      if (Math.abs(x) <= 1 && Math.abs(y) <= 1) st.pts.push([x, y, 1]);
    }
  }
  st.pts.forEach(p => p[2] *= st.playing ? 0.965 : 1);
  st.pts = st.pts.filter(p => p[2] > 0.04).slice(-1600);
  st.pts.forEach(([x, y, a]) => { cg.fillStyle = `rgba(243,179,61,${a.toFixed(3)})`; cg.fillRect(cx + x * s - 1, cy - y * s - 1, 2, 2); });
  $('constCap').textContent = `phase · ${fmtMs(st.pos)}${inBurst() ? '' : ' · no lock'}`;
}

/* ---- splitters ---- */
function splitter(el, onMove) {
  el.addEventListener('pointerdown', e => { el.setPointerCapture(e.pointerId); el.classList.add('drag');
    const mv = ev => onMove(ev), up = () => { el.classList.remove('drag'); el.removeEventListener('pointermove', mv); el.removeEventListener('pointerup', up); };
    el.addEventListener('pointermove', mv); el.addEventListener('pointerup', up); });
}
const root = document.documentElement;
splitter($('vsplit'), e => { const w = Math.max(300, Math.min(innerWidth * .6, innerWidth - e.clientX)); root.style.setProperty('--right-w', w + 'px'); resize(); });
splitter($('hsplit'), e => { const right = document.querySelector('.right').getBoundingClientRect();
  const h = Math.max(120, Math.min(right.height - 140, right.bottom - e.clientY)); root.style.setProperty('--bottom-h', h + 'px'); resize(); });

/* ---- loop ---- */
let last = performance.now(), uiTick = 0;
function frame(now) {
  const dt = Math.min(100, now - last); last = now;
  if (st.playing && st.pos < DUR) {
    st.pos = Math.min(DUR, st.pos + dt / 1000 * st.speed);
    if (st.pos >= DUR) { st.playing = false; pushLog(DUR, 'end of recording, processing finished'); renderLog(); renderChrome(); }
    if (st.follow) st.viewStart = clampView(st.pos - st.viewSpan * 0.72);
    if (advanceTo(st.pos)) { renderTabs(); renderResults(); renderLog(); }
  }
  uiTick += dt;
  if (uiTick > 250) { uiTick = 0;
    if (st.results.some(r => r.live)) renderResults();
    $('stPos').textContent = fmt(st.pos); $('stDur').textContent = fmt(DUR);
    $('stProg').style.width = (st.pos / DUR * 100).toFixed(2) + '%';
    if (st.speed === 48 && st.playing) $('stRate').textContent = `×${(40 + Math.random()*9).toFixed(1)} (max)`;
  }
  drawPlot(); drawConst(dt);
  requestAnimationFrame(frame);
}
new ResizeObserver(resize).observe($('plot'));
new ResizeObserver(resizeConst).observe($('const'));
setDmd('Navtex', true); st.playing = true; renderChrome(); resize();
requestAnimationFrame(frame);
})();
