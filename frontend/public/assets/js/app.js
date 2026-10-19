/* Mailvora progressive enhancement.
   Every page works without JavaScript; this file only improves it.
   No inline handlers: behaviour is attached through data-* attributes. */
(function () {
  'use strict';

  var csrf = (document.querySelector('meta[name="csrf-token"]') || {}).content || '';
  var $ = function (sel, root) { return (root || document).querySelector(sel); };
  var $$ = function (sel, root) { return Array.prototype.slice.call((root || document).querySelectorAll(sel)); };

  function store(key, value) {
    try { if (value === undefined) return localStorage.getItem(key); localStorage.setItem(key, value); } catch (e) { return null; }
    return null;
  }

  function post(url, data, asJson) {
    var body = data instanceof FormData ? data : new URLSearchParams(data || {});
    return fetch(url, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'X-CSRF-Token': csrf, 'X-Requested-With': 'fetch', 'Accept': asJson ? 'application/json' : 'text/html' },
      body: body
    });
  }

  /* ── theme: light / dark / follow the device ───────────────────────── */
  function applyTheme(choice) {
    var root = document.documentElement;
    if (choice === 'light' || choice === 'dark') { root.setAttribute('data-theme', choice); store('mv-theme', choice); }
    else { root.removeAttribute('data-theme'); try { localStorage.removeItem('mv-theme'); } catch (e) { /* ignore */ } }
    $$('[data-theme-set]').forEach(function (b) {
      b.setAttribute('aria-checked', b.getAttribute('data-theme-set') === (choice || 'system') ? 'true' : 'false');
    });
  }
  var savedTheme = store('mv-theme');
  applyTheme(savedTheme === 'light' || savedTheme === 'dark' ? savedTheme : 'system');
  $$('[data-theme-set]').forEach(function (btn) {
    btn.addEventListener('click', function () { applyTheme(btn.getAttribute('data-theme-set')); });
  });
  $$('[data-theme-toggle]').forEach(function (btn) {
    btn.addEventListener('click', function () {
      var current = document.documentElement.getAttribute('data-theme') ||
        (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
      applyTheme(current === 'dark' ? 'light' : 'dark');
    });
  });

  /* ── tools hub search ────────────────────────────────────────────── */
  var toolSearch = $('[data-tool-filter]');
  if (toolSearch) {
    toolSearch.addEventListener('input', function () {
      var q = toolSearch.value.trim().toLowerCase(), shown = 0;
      $$('[data-tool-card]').forEach(function (card) {
        var hit = q === '' || card.getAttribute('data-tool-card').indexOf(q) !== -1 || card.textContent.toLowerCase().indexOf(q) !== -1;
        card.hidden = !hit; if (hit) shown += 1;
      });
      var empty = $('[data-tool-empty]'); if (empty) empty.hidden = shown !== 0;
    });
  }

  /* ── help widget: close on Escape or outside click ───────────────── */
  $$('.help-widget').forEach(function (w) {
    document.addEventListener('keydown', function (e) { if (e.key === 'Escape') w.removeAttribute('open'); });
    document.addEventListener('click', function (e) { if (w.hasAttribute('open') && !w.contains(e.target)) w.removeAttribute('open'); });
  });

  /* ── mobile navigation ───────────────────────────────────────────── */
  $$('[data-nav-toggle]').forEach(function (btn) {
    btn.addEventListener('click', function () {
      var shell = btn.closest('.shell, .mail-shell') || $('.shell, .mail-shell');
      if (!shell) return;
      var open = shell.classList.toggle('nav-open');
      btn.setAttribute('aria-expanded', open ? 'true' : 'false');
    });
  });
  document.addEventListener('click', function (e) {
    var shell = $('.shell.nav-open, .mail-shell.nav-open');
    if (shell && !e.target.closest('.sidebar, .mail-side, [data-nav-toggle]')) shell.classList.remove('nav-open');
  });

  /* ── dropdowns: close on outside click / Escape ──────────────────── */
  document.addEventListener('click', function (e) {
    $$('details.dropdown[open]').forEach(function (d) { if (!d.contains(e.target)) d.removeAttribute('open'); });
  });
  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape') $$('details.dropdown[open]').forEach(function (d) { d.removeAttribute('open'); });
  });

  /* ── copy buttons ────────────────────────────────────────────────── */
  function copyText(text) {
    if (navigator.clipboard && window.isSecureContext) return navigator.clipboard.writeText(text);
    return new Promise(function (resolve, reject) {
      var ta = document.createElement('textarea');
      ta.value = text; ta.setAttribute('readonly', ''); ta.style.position = 'fixed'; ta.style.opacity = '0';
      document.body.appendChild(ta); ta.select();
      try { document.execCommand('copy') ? resolve() : reject(); } catch (err) { reject(err); }
      document.body.removeChild(ta);
    });
  }
  document.addEventListener('click', function (e) {
    var btn = e.target.closest('[data-copy], [data-copy-target]');
    if (!btn) return;
    e.preventDefault();
    var text = btn.getAttribute('data-copy');
    if (text === null) {
      var target = document.getElementById(btn.getAttribute('data-copy-target'));
      text = target ? (target.value !== undefined && target.tagName !== 'DIV' ? target.value : target.textContent.trim()) : '';
    }
    var label = btn.querySelector('.copy-label');
    var original = label ? label.textContent : null;
    copyText(text).then(function () {
      btn.classList.add('is-copied');
      if (label) label.textContent = 'Copied';
      announce('Copied to clipboard');
      setTimeout(function () { btn.classList.remove('is-copied'); if (label) label.textContent = original; }, 1800);
    }, function () { announce('Copy failed. Select the text and copy it manually.'); });
  });

  /* Polite live region for status announcements. */
  var live = document.createElement('div');
  live.className = 'sr-only'; live.setAttribute('aria-live', 'polite'); live.setAttribute('role', 'status');
  document.body.appendChild(live);
  function announce(msg) { live.textContent = ''; setTimeout(function () { live.textContent = msg; }, 30); }

  /* ── password reveal ─────────────────────────────────────────────── */
  $$('[data-reveal]').forEach(function (btn) {
    btn.addEventListener('click', function () {
      var input = document.getElementById(btn.getAttribute('data-reveal'));
      if (!input) return;
      var show = input.type === 'password';
      input.type = show ? 'text' : 'password';
      btn.setAttribute('aria-pressed', show ? 'true' : 'false');
      btn.setAttribute('aria-label', show ? 'Hide password' : 'Show password');
    });
  });

  /* ── confirmation dialog for destructive forms ───────────────────── */
  var dialog = $('#confirm-dialog');
  document.addEventListener('submit', function (e) {
    var form = e.target;
    if (!(form instanceof HTMLFormElement)) return;
    if (form.hasAttribute('data-confirm') && !form.dataset.confirmed && dialog && typeof dialog.showModal === 'function') {
      e.preventDefault();
      $('#confirm-title', dialog).textContent = form.getAttribute('data-confirm-title') || 'Are you sure?';
      $('#confirm-text', dialog).textContent = form.getAttribute('data-confirm');
      var ok = $('#confirm-ok', dialog);
      ok.textContent = form.getAttribute('data-confirm-label') || 'Confirm';
      ok.className = 'btn ' + (form.hasAttribute('data-confirm-safe') ? 'btn-primary' : 'btn-danger');
      dialog.returnValue = '';
      dialog.showModal();
      dialog.addEventListener('close', function onClose() {
        dialog.removeEventListener('close', onClose);
        if (dialog.returnValue === 'ok') { form.dataset.confirmed = '1'; form.requestSubmit ? form.requestSubmit(e.submitter || undefined) : form.submit(); }
      });
      return;
    }
    // Prevent accidental double submission; show progress on the button.
    if (form.dataset.submitting && !form.hasAttribute('data-allow-resubmit')) { e.preventDefault(); return; }
    if (form.method.toLowerCase() === 'post' && !form.hasAttribute('data-no-busy')) {
      form.dataset.submitting = '1';
      var btn = e.submitter || $('button[type=submit], button:not([type])', form);
      if (btn) { btn.classList.add('is-busy'); btn.setAttribute('aria-busy', 'true'); }
      if (btn && btn.name) {
        // Keep the submitter's value: disabled buttons are not submitted.
        var hidden = document.createElement('input');
        hidden.type = 'hidden'; hidden.name = btn.name; hidden.value = btn.value;
        form.appendChild(hidden);
      }
      setTimeout(function () { delete form.dataset.submitting; if (btn) { btn.classList.remove('is-busy'); btn.removeAttribute('aria-busy'); } }, 15000);
    }
  }, true);

  /* ── flash messages ──────────────────────────────────────────────── */
  $$('.flash-stack .alert').forEach(function (el) {
    var close = $('.dismiss', el);
    if (close) close.addEventListener('click', function () { el.remove(); });
    if (el.classList.contains('alert-success') || el.classList.contains('alert-info')) {
      setTimeout(function () { el.style.transition = 'opacity .3s'; el.style.opacity = '0'; setTimeout(function () { el.remove(); }, 320); }, 7000);
    }
  });

  /* ── tabs ────────────────────────────────────────────────────────── */
  $$('[data-tabs]').forEach(function (group) {
    var tabs = $$('[role=tab]', group);
    tabs.forEach(function (tab) {
      tab.addEventListener('click', function () {
        tabs.forEach(function (t) {
          var selected = t === tab;
          t.setAttribute('aria-selected', selected ? 'true' : 'false');
          t.classList.toggle('is-active', selected);
          var panel = document.getElementById(t.getAttribute('aria-controls'));
          if (panel) panel.hidden = !selected;
        });
        if (group.dataset.tabs) store('mv-tab-' + group.dataset.tabs, tab.id);
      });
    });
    var saved = group.dataset.tabs && store('mv-tab-' + group.dataset.tabs);
    var initial = saved && document.getElementById(saved);
    if (initial && group.contains(initial)) initial.click();
  });

  /* ── select all / bulk actions (webmail, tables) ─────────────────── */
  $$('[data-select-all]').forEach(function (master) {
    var scope = document.getElementById(master.getAttribute('data-select-all')) || document;
    var boxes = function () { return $$('input[type=checkbox][data-select-item]', scope); };
    var bar = $('[data-bulk-bar]');
    function refresh() {
      var picked = boxes().filter(function (b) { return b.checked; }).length;
      master.checked = picked > 0 && picked === boxes().length;
      master.indeterminate = picked > 0 && picked < boxes().length;
      if (bar) { bar.hidden = picked === 0; var c = $('[data-bulk-count]', bar); if (c) c.textContent = picked + ' selected'; }
    }
    master.addEventListener('change', function () { boxes().forEach(function (b) { b.checked = master.checked; }); refresh(); });
    scope.addEventListener('change', function (e) { if (e.target.matches('[data-select-item]')) refresh(); });
    refresh();
  });

  /* ── keyboard shortcuts (webmail) ────────────────────────────────── */
  if (document.body.hasAttribute('data-shortcuts')) {
    document.addEventListener('keydown', function (e) {
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      var t = e.target;
      if (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName)) return;
      var go = function (sel) { var el = $(sel); if (el) { e.preventDefault(); el.click(); } };
      switch (e.key) {
        case 'c': go('[data-shortcut="compose"]'); break;
        case '/': { var s = $('[data-shortcut="search"]'); if (s) { e.preventDefault(); s.focus(); } break; }
        case 'j': go('[data-shortcut="next"]'); break;
        case 'k': go('[data-shortcut="prev"]'); break;
        case 'r': go('[data-shortcut="reply"]'); break;
        case 'a': go('[data-shortcut="reply-all"]'); break;
        case 'f': go('[data-shortcut="forward"]'); break;
        case 'e': go('[data-shortcut="archive"]'); break;
        case '#': case 'Delete': go('[data-shortcut="delete"]'); break;
        case 'u': go('[data-shortcut="back"]'); break;
        case '?': go('[data-shortcut="help"]'); break;
      }
    });
  }

  /* ── email body iframe: size to content (same-origin, scripts disabled) */
  $$('iframe[data-autosize]').forEach(function (frame) {
    function fit() {
      try {
        var doc = frame.contentDocument;
        if (doc && doc.body) frame.style.height = Math.max(240, doc.documentElement.scrollHeight + 8) + 'px';
      } catch (e) { /* cross-origin: keep CSS height */ }
    }
    frame.addEventListener('load', function () { fit(); setTimeout(fit, 400); });
  });

  /* ── recipient autocomplete (webmail compose) ────────────────────── */
  $$('input[data-suggest]').forEach(function (input) {
    var list = document.getElementById(input.getAttribute('list'));
    if (!list) return;
    var timer, lastQuery = '';
    input.addEventListener('input', function () {
      clearTimeout(timer);
      var parts = input.value.split(/[,;]/);
      var term = parts[parts.length - 1].trim();
      if (term.length < 2 || term === lastQuery) return;
      timer = setTimeout(function () {
        lastQuery = term;
        var base = input.getAttribute('data-suggest');
        fetch(base + (base.indexOf('?') === -1 ? '?' : '&') + 'q=' + encodeURIComponent(term), { credentials: 'same-origin', headers: { 'Accept': 'application/json' } })
          .then(function (r) { return r.ok ? r.json() : { data: [] }; })
          .then(function (res) {
            var prefix = parts.slice(0, -1).map(function (p) { return p.trim(); }).filter(Boolean);
            list.innerHTML = '';
            (res.data || []).forEach(function (s) {
              var opt = document.createElement('option');
              var addr = s.name ? s.name + ' <' + s.email + '>' : s.email;
              opt.value = prefix.concat([addr]).join(', ');
              opt.label = (s.source === 'colleague' ? 'Colleague · ' : s.source === 'contact' ? 'Contact · ' : 'Recent · ') + s.email;
              list.appendChild(opt);
            });
          }).catch(function () {});
      }, 180);
    });
  });

  /* ── template live preview ───────────────────────────────────────── */
  var editor = $('[data-template-editor]');
  if (editor) {
    var frame = $('#preview-frame');
    var subjectOut = $('#preview-subject');
    var textOut = $('#preview-text');
    var errorOut = $('#preview-error');
    var pending;
    var render = function () {
      var fd = new FormData(editor);
      post(editor.getAttribute('data-preview-url'), fd, true)
        .then(function (r) { return r.json().then(function (j) { return { ok: r.ok, body: j }; }); })
        .then(function (res) {
          if (!res.ok) { if (errorOut) { errorOut.hidden = false; errorOut.textContent = (res.body.error && res.body.error.message) || 'Preview failed'; } return; }
          if (errorOut) errorOut.hidden = true;
          if (subjectOut) subjectOut.textContent = res.body.subject || '';
          if (frame) frame.srcdoc = res.body.html || '<p style="font-family:sans-serif;color:#667">No HTML version.</p>';
          if (textOut) textOut.textContent = res.body.text || '(No plain-text version; one is recommended for accessibility and deliverability.)';
        }).catch(function () {});
    };
    editor.addEventListener('input', function () { clearTimeout(pending); pending = setTimeout(render, 450); });
    $$('[data-preview-width]').forEach(function (b) {
      b.addEventListener('click', function () {
        $$('[data-preview-width]').forEach(function (x) { x.setAttribute('aria-pressed', x === b ? 'true' : 'false'); });
        if (frame) frame.classList.toggle('preview-mobile', b.getAttribute('data-preview-width') === 'mobile');
      });
    });
    render();
  }

  /* ── setup guide: analytics beacon on step views ─────────────────── */
  var stepEl = $('[data-setup-step]');
  if (stepEl && stepEl.getAttribute('data-event-url')) {
    post(stepEl.getAttribute('data-event-url'), {
      event: 'step_viewed', client: stepEl.getAttribute('data-client') || '', device: stepEl.getAttribute('data-device') || '',
      detail: 'step_' + stepEl.getAttribute('data-setup-step')
    }, true).catch(function () {});
  }
  $$('details.cant-find[data-help-topic]').forEach(function (d) {
    d.addEventListener('toggle', function () {
      if (!d.open || d.dataset.sent) return;
      d.dataset.sent = '1';
      var url = d.getAttribute('data-event-url');
      if (url) post(url, { event: 'help_viewed', client: d.getAttribute('data-client') || '', detail: d.getAttribute('data-help-topic') }, true).catch(function () {});
    });
  });

  /* ── outgoing connection test: poll until the message arrives ───── */
  var poller = $('[data-poll-outgoing]');
  if (poller) {
    var attempts = 0;
    var status = $('[data-poll-status]');
    var tick = function () {
      attempts++;
      post(poller.getAttribute('data-poll-outgoing'), {}, true)
        .then(function (r) { return r.json(); })
        .then(function (res) {
          if (res.verified) { window.location.href = poller.getAttribute('data-done-url'); return; }
          if (status) status.textContent = 'Waiting for your message… (checked ' + attempts + ' time' + (attempts === 1 ? '' : 's') + ')';
          if (attempts < 18) setTimeout(tick, 10000);
          else if (status) status.textContent = 'The message has not arrived yet. Check that it was sent from your work account, then press "Check again".';
        }).catch(function () { if (attempts < 18) setTimeout(tick, 15000); });
    };
    setTimeout(tick, 8000);
  }

  /* ── auto-submit selects (filters) ───────────────────────────────── */
  $$('[data-autosubmit]').forEach(function (el) {
    el.addEventListener('change', function () { if (el.form) el.form.requestSubmit ? el.form.requestSubmit() : el.form.submit(); });
  });
})();
