// Small progressive enhancements: mobile menu, copy buttons, and offline search.
(function () {
  'use strict';
  var toggle = document.querySelector('.menu-toggle');
  var sidebar = document.getElementById('sidebar');
  if (toggle && sidebar) {
    toggle.addEventListener('click', function () {
      var open = sidebar.classList.toggle('open');
      toggle.setAttribute('aria-expanded', open ? 'true' : 'false');
    });
  }

  document.querySelectorAll('pre').forEach(function (pre) {
    if (!navigator.clipboard) { return; }
    var button = document.createElement('button');
    button.className = 'copy';
    button.type = 'button';
    button.textContent = 'Copy';
    button.addEventListener('click', function () {
      var code = pre.querySelector('code');
      navigator.clipboard.writeText((code || pre).innerText.replace(/\n$/, '')).then(function () {
        button.textContent = 'Copied';
        setTimeout(function () { button.textContent = 'Copy'; }, 1600);
      });
    });
    pre.appendChild(button);
  });

  var input = document.getElementById('search');
  var results = document.getElementById('results');
  if (!input || !results) { return; }
  var index = null;
  function load() {
    if (index) { return Promise.resolve(index); }
    return fetch('assets/search-index.json').then(function (r) { return r.json(); }).then(function (data) { index = data; return data; });
  }
  function score(page, words) {
    var title = page.title.toLowerCase();
    var heads = page.headings.join(' ').toLowerCase();
    var text = page.text.toLowerCase();
    var total = 0;
    for (var i = 0; i < words.length; i++) {
      var word = words[i];
      var found = 0;
      if (title.indexOf(word) >= 0) { found += 8; }
      if (heads.indexOf(word) >= 0) { found += 4; }
      if (text.indexOf(word) >= 0) { found += 1; }
      if (!found) { return 0; }
      total += found;
    }
    return total;
  }
  function snippet(page, words) {
    var text = page.text;
    var at = text.toLowerCase().indexOf(words[0]);
    if (at < 0) { return page.summary; }
    var start = Math.max(0, at - 40);
    return (start ? '…' : '') + text.slice(start, at + 90) + '…';
  }
  function render(list, words) {
    results.textContent = '';
    if (!list.length) {
      var none = document.createElement('li');
      none.textContent = 'No matches';
      none.style.padding = '8px 10px';
      results.appendChild(none);
    }
    list.slice(0, 8).forEach(function (page) {
      var item = document.createElement('li');
      var link = document.createElement('a');
      link.href = page.url;
      link.textContent = page.title;
      var small = document.createElement('small');
      small.textContent = snippet(page, words);
      link.appendChild(small);
      item.appendChild(link);
      results.appendChild(item);
    });
    results.hidden = false;
  }
  input.addEventListener('input', function () {
    var words = input.value.toLowerCase().split(/\s+/).filter(Boolean);
    if (!words.length) { results.hidden = true; return; }
    load().then(function (pages) {
      var ranked = pages.map(function (page) { return { page: page, score: score(page, words) }; })
        .filter(function (entry) { return entry.score > 0; })
        .sort(function (a, b) { return b.score - a.score; })
        .map(function (entry) { return entry.page; });
      render(ranked, words);
    });
  });
  document.addEventListener('click', function (event) {
    if (!results.contains(event.target) && event.target !== input) { results.hidden = true; }
  });
  document.addEventListener('keydown', function (event) {
    if (event.key === 'Escape') { results.hidden = true; input.blur(); }
    if (event.key === '/' && document.activeElement !== input && !/INPUT|TEXTAREA/.test(document.activeElement.tagName)) {
      event.preventDefault(); input.focus();
    }
  });
})();
