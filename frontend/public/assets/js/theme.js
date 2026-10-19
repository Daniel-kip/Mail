/* Applies the saved colour theme before first paint (loaded synchronously
   in <head>; external file because the CSP forbids inline scripts). */
(function () {
  try {
    var t = localStorage.getItem('mv-theme');
    if (t === 'light' || t === 'dark') document.documentElement.setAttribute('data-theme', t);
  } catch (e) { /* storage unavailable: follow the system theme */ }
})();
