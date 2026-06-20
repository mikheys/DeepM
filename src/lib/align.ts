// Post-hoc alignment between source text and its translation, computed entirely
// in the frontend (no backend / model / pipeline changes). Two granularities:
// sentences (via Intl.Segmenter, which knows RU & EN) and paragraphs (blank
// lines). Pairing is by index — translation almost always preserves segment
// order and count; when counts differ we simply pair the overlap and leave the
// rest unmatched (surfaced in the debug overlay).

export type LinkMode = "off" | "sentence" | "paragraph";

/** Split text into segments of the requested granularity. */
export function segment(text: string, lang: string, mode: LinkMode): string[] {
  if (mode === "off" || !text.trim()) return [];

  if (mode === "paragraph") {
    return text.split(/\n\s*\n/).map((s) => s.trim()).filter(Boolean);
  }

  // Sentences. Intl.Segmenter (WebView2/Chromium) handles abbreviations and
  // both scripts far better than a regex; fall back to a regex if unavailable.
  const Segmenter = (Intl as unknown as { Segmenter?: any }).Segmenter;
  if (Segmenter) {
    try {
      const seg = new Segmenter(lang || "en", { granularity: "sentence" });
      const out: string[] = [];
      for (const part of seg.segment(text)) {
        const s = String(part.segment).trim();
        if (s) out.push(s);
      }
      if (out.length) return out;
    } catch {
      /* fall through to regex */
    }
  }
  return text.split(/(?<=[.!?…])\s+/).map((s) => s.trim()).filter(Boolean);
}

/** Index of the segment paired with `i` on the other side, or null if none. */
export function pairedIndex(i: number, otherLength: number): number | null {
  return i >= 0 && i < otherLength ? i : null;
}
