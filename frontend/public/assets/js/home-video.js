/* Landing-page hero video: plays automatically, muted, in an endless loop,
   with no controls. If the browser pauses it (tab switch, power saving,
   autoplay blocked until the first tap), playback resumes on its own.
   Visitors who ask their device for reduced motion see the still poster. */
(function () {
  'use strict';
  var video = document.querySelector('.hero-network-video');
  if (!video) return;
  var reduced = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  if (reduced) { video.removeAttribute('autoplay'); video.pause(); return; }

  video.muted = true;
  video.loop = true;
  function play() {
    if (!video.paused) return;
    var p = video.play();
    if (p && p.catch) p.catch(function () { /* retried on the next trigger */ });
  }
  play();
  video.addEventListener('pause', function () { if (!document.hidden) setTimeout(play, 150); });
  video.addEventListener('ended', play);
  video.addEventListener('canplay', play);
  document.addEventListener('visibilitychange', function () { if (!document.hidden) play(); });
  ['pointerdown', 'touchstart', 'keydown', 'scroll'].forEach(function (type) {
    window.addEventListener(type, play, { passive: true });
  });
  video.addEventListener('contextmenu', function (event) { event.preventDefault(); });
  video.addEventListener('dragstart', function (event) { event.preventDefault(); });
})();
