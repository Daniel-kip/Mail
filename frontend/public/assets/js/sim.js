/* Landing-page send simulation: types a request, flies the message through
   acceptance, DKIM signing and TLS relay, and lands it on a phone.
   Pure progressive enhancement: without JavaScript (or with reduced motion)
   the final, delivered state is shown statically. */
(function () {
  'use strict';
  var root = document.querySelector('[data-sim]');
  if (!root) return;

  var api = (root.getAttribute('data-api-url') || 'https://api.example.com').replace(/\/$/, '');
  var SCENARIOS = [
    {
      file: 'POST /v1/messages',
      code: 'curl ' + api + '/v1/messages \\\n' +
        '  -H "Authorization: Bearer mv_live_••••" \\\n' +
        '  -H "Idempotency-Key: inv-1042" \\\n' +
        '  -d \'{\n' +
        '    "from": "Acme Store <billing@acme.co.ke>",\n' +
        '    "to": "wanjiru@gmail.com",\n' +
        '    "template": "invoice",\n' +
        '    "variables": { "number": "INV-1042" }\n' +
        '  }\'',
      from: 'Acme Store', subject: 'Invoice INV-1042', preview: 'KES 1,200 paid via M-Pesa. Thank you!',
      log: ['202 accepted · msg_01J8…K2', 'template "invoice" v3 rendered', 'DKIM d=acme.co.ke s=mv2026 signed', 'STARTTLS → gmail-smtp-in.l.google.com', '250 2.0.0 OK · delivered'],
      ms: [12, 31, 420, 1380]
    },
    {
      file: 'Webmail · New message',
      code: 'From:    principal@riverside.ac.ke\n' +
        'To:      staff@riverside.ac.ke\n' +
        'Subject: Staff meeting at 2 pm\n\n' +
        'Good morning team,\n' +
        'We meet in the library at 2 pm.\n' +
        'Agenda attached.\n\n' +
        '— Grace, Principal',
      from: 'Principal · Riverside', subject: 'Staff meeting at 2 pm', preview: 'We meet in the library at 2 pm. Agenda attached.',
      log: ['submitted · 465 implicit TLS', 'alias staff@ → 38 mailboxes', 'DKIM d=riverside.ac.ke signed', 'LMTP → mailbox storage', 'delivered to 38 inboxes'],
      ms: [8, 24, 96, 640]
    },
    {
      file: 'POST /v1/messages',
      code: 'await mailvora.send({\n' +
        '  from: "Duka <no-reply@duka.co.ke>",\n' +
        '  to: user.email,\n' +
        '  subject: "Your sign-in code",\n' +
        '  text: `Your code is ${code}`,\n' +
        '  tags: ["otp"],\n' +
        '});',
      from: 'Duka', subject: 'Your sign-in code: 482 913', preview: 'It expires in 10 minutes. Never share it.',
      log: ['202 accepted · priority queue', 'suppression list checked', 'DKIM d=duka.co.ke signed', 'TLS 1.3 → outlook.com', '250 OK · delivered'],
      ms: [9, 22, 310, 1120]
    }
  ];

  var $ = function (sel) { return root.querySelector(sel); };
  var $$ = function (sel) { return Array.prototype.slice.call(root.querySelectorAll(sel)); };
  var reduced = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  var codeEl = $('[data-sim-code]'), logEl = $('[data-sim-log]'), env = $('[data-sim-envelope]');
  var steps = $$('[data-sim-track] li'), tabs = $$('[data-sim-tab]');
  var timers = [], typing = null, current = 0, sent = 0, visible = true, playing = false;

  function later(fn, ms) { timers.push(setTimeout(fn, ms)); }
  function clear() {
    timers.forEach(clearTimeout);
    timers = [];
    if (typing) { clearInterval(typing); typing = null; }
  }
  function text(sel, value) { var el = $(sel); if (el) el.textContent = value; }

  function reset(s) {
    clear();
    root.classList.remove('is-sending', 'is-delivered');
    codeEl.textContent = '';
    logEl.innerHTML = '';
    steps.forEach(function (li) { li.classList.remove('is-done', 'is-active'); li.querySelector('[data-sim-ms]').innerHTML = '&nbsp;'; });
    env.style.setProperty('--sim-pos', '0');
    text('[data-sim-file]', s.file);
    text('[data-sim-from]', s.from); text('[data-sim-toast-from]', s.from);
    text('[data-sim-subject]', s.subject); text('[data-sim-toast-subject]', s.subject);
    text('[data-sim-preview]', s.preview); text('[data-sim-avatar]', s.from.charAt(0));
    var now = new Date();
    text('[data-sim-clock]', now.getHours() + ':' + String(now.getMinutes()).padStart(2, '0'));
    tabs.forEach(function (t, i) { t.classList.toggle('is-active', i === current); t.setAttribute('aria-selected', i === current ? 'true' : 'false'); });
  }

  function logLine(line, delay) {
    later(function () {
      var row = document.createElement('div');
      row.className = 'sim-log-line';
      var t = new Date();
      row.textContent = t.toTimeString().slice(0, 8) + '  ' + line;
      logEl.appendChild(row);
      while (logEl.children.length > 5) logEl.removeChild(logEl.firstChild);
    }, delay);
  }

  function finalState(s) {
    reset(s);
    codeEl.textContent = s.code;
    steps.forEach(function (li, i) { li.classList.add('is-done'); li.querySelector('[data-sim-ms]').textContent = s.ms[i] + ' ms'; });
    env.style.setProperty('--sim-pos', '1');
    s.log.forEach(function (l) { var r = document.createElement('div'); r.className = 'sim-log-line'; r.textContent = l; logEl.appendChild(r); });
    root.classList.add('is-delivered');
    text('[data-sim-count="latency"]', (s.ms[3] / 1000).toFixed(1) + 's');
  }

  function play(index) {
    current = index;
    var s = SCENARIOS[index];
    if (reduced) { finalState(s); return; }
    playing = true;
    reset(s);
    var chars = s.code.length, typeMs = Math.min(1900, chars * 14), i = 0;
    typing = setInterval(function () {
      i += Math.max(1, Math.round(chars / (typeMs / 16)));
      codeEl.textContent = s.code.slice(0, i);
      if (i >= chars) { clearInterval(typing); typing = null; }
    }, 16);
    var t = typeMs + 250;
    later(function () { root.classList.add('is-sending'); logLine(s.log[0], 0); }, t);
    steps.forEach(function (li, n) {
      later(function () { li.classList.add('is-active'); env.style.setProperty('--sim-pos', String((n + 0.5) / steps.length)); }, t + 150 + n * 650);
      later(function () {
        li.classList.remove('is-active'); li.classList.add('is-done');
        li.querySelector('[data-sim-ms]').textContent = s.ms[n] + ' ms';
        if (n + 1 < s.log.length) logLine(s.log[n + 1], 0);
      }, t + 650 + n * 650);
    });
    var end = t + 650 + steps.length * 650;
    later(function () {
      env.style.setProperty('--sim-pos', '1');
      root.classList.remove('is-sending'); root.classList.add('is-delivered');
      sent += 1;
      text('[data-sim-count="sent"]', String(sent));
      text('[data-sim-count="latency"]', (s.ms[3] / 1000).toFixed(1) + 's');
    }, end);
    later(function () { playing = false; if (visible) play((index + 1) % SCENARIOS.length); }, end + 3600);
  }

  tabs.forEach(function (tab, i) { tab.addEventListener('click', function () { play(i); }); });
  var replay = $('[data-sim-replay]');
  if (replay) replay.addEventListener('click', function () { play(current); });

  if ('IntersectionObserver' in window) {
    new IntersectionObserver(function (entries) {
      visible = entries[0].isIntersecting;
      if (visible && !playing) play(current);
      if (!visible) { clear(); playing = false; }
    }, { threshold: 0.25 }).observe(root);
  } else {
    play(0);
  }
  document.addEventListener('visibilitychange', function () {
    if (document.hidden) { clear(); playing = false; } else if (visible && !playing) { play(current); }
  });
})();
