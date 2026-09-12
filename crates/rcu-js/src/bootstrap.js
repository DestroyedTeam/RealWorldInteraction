// Runs only in the owned QuickJS process. Capture the tiny native bridge, then
// remove its globals. No Node/process/filesystem/network/module loader exists.
(() => {
  const call = globalThis.__rcuCall;
  const send = globalThis.__rcuEmit;
  const image = globalThis.__rcuImage;
  const stringify = JSON.stringify;
  const parse = JSON.parse;
  delete globalThis.__rcuCall;
  delete globalThis.__rcuEmit;
  delete globalThis.__rcuImage;
  Object.defineProperties(globalThis, {
    state: { value: Object.create(null), writable: false },
    rcu: { value: new RcuClient((tool, args) => parse(call(tool, stringify(args)))), writable: false },
    emit: { value: value => send(stringify(value ?? null)), writable: false },
    emitImage: { value: media => {
      if (media?.type !== 'image' || typeof media.data !== 'string') throw new TypeError('Pass an image from reply.media');
      image(media.data, media.mimeType);
    }, writable: false },
  });
})();
