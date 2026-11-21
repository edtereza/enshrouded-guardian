// Resolve image URLs against this shared catalog, both on file:// and HTTP.
(() => {
  const script = new URL(document.currentScript.src);
  const base = new URL('.', script);
  const key = script.searchParams.get('key');
  for (const table of [window.guardianCatalogs.icons, window.guardianCatalogs.ui]) {
    for (const name of Object.keys(table)) {
      const url = new URL(table[name], base);
      if (key) url.searchParams.set('key', key);
      table[name] = url.href;
    }
  }
})();
