exports.address = value => {
  if (typeof value !== 'string' || !value.trim()) return null;
  let input = value.trim();
  if (!/^[a-z][a-z\d+.-]*:/i.test(input)) input = `https://${input}`;
  try {
    const url = new URL(input);
    return ['https:', 'http:'].includes(url.protocol) ? url.href : null;
  } catch { return null; }
};
exports.theme = source => {
  const colors = {background:'#1a1b26', foreground:'#c0caf5', accent:'#7aa2f7', muted:'#565f89', lighter_background:'#24283b', mode:'dark'};
  for (const [, key, value] of source.matchAll(/^\s*(\w+)\s*=\s*["']([^"']+)["']/gm)) {
    if (key === 'mode' && ['dark', 'light'].includes(value)) colors.mode = value;
    else if (key in colors && /^#[a-f\d]{6}$/i.test(value)) colors[key] = value;
  }
  return colors;
};
