const fs = require('node:fs');
const path = require('node:path');

// Watch directory entries, not a file inode: editors and theme managers rename
// files/directories and switch symlinks. Ancestors also cover initially missing paths.
function watchTheme(files, onChange, {debounceMs = 50} = {}) {
  let closed = false;
  let pending;
  let watchers = [];
  function arm() {
    for (const watcher of watchers) watcher.close();
    watchers = [];
    const entries = new Map();
    function addChain(file) {
      let child = path.resolve(file);
      while (true) {
        const parent = path.dirname(child);
        if (parent === child) break;
        try {
          const resolved = fs.realpathSync(parent);
          if (fs.statSync(resolved).isDirectory()) {
            if (!entries.has(resolved)) entries.set(resolved, new Set());
            entries.get(resolved).add(path.basename(child));
          }
        } catch {} // Missing/inaccessible descendants are covered by existing ancestors.
        child = parent;
      }
    }
    for (const file of files) {
      addChain(file);
      // Each resolved prefix may escape the lexical tree through a symlink.
      // Watch its target ancestors as well, including replacement of that target.
      let prefix = path.resolve(file);
      while (true) {
        try { addChain(fs.realpathSync(prefix)); } catch {}
        const parent = path.dirname(prefix);
        if (parent === prefix) break;
        prefix = parent;
      }
    }
    for (const [directory, names] of entries) {
      try {
        const watcher = fs.watch(directory, {persistent:false}, (_event, filename) => {
          if (filename === null || names.has(String(filename))) schedule();
        });
        watcher.on('error', schedule);
        watchers.push(watcher);
      } catch {} // No retry loop on unavailable watches; another ancestor event can recover.
    }
  }
  function schedule() {
    if (closed) return;
    clearTimeout(pending);
    pending = setTimeout(() => {
      pending = undefined;
      if (closed) return;
      arm();
      onChange();
    }, debounceMs);
    pending.unref();
  }
  arm();
  return {close() {
    closed = true;
    clearTimeout(pending);
    for (const watcher of watchers) watcher.close();
    watchers = [];
  }};
}
module.exports = {watchTheme};
