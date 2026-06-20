// Post-hoc alignment between source text and its translation, fully in the
// frontend (no backend / model / pipeline changes).
//
// Segments carry their character OFFSETS into the original text, so the UI can
// render the text verbatim (every blank line / space preserved) and highlight a
// segment by its range — the source stays a real, editable <textarea>.
//
// Sentences would drift with plain index pairing (one merged/split sentence
// shifts everything after it), so we align by length with dynamic programming
// (Gale–Church), allowing 1:1, 1:2, 2:1 and skip beads, and bound the drift to
// within a paragraph.

export type LinkMode = "off" | "sentence" | "paragraph";

/** A segment of the original text: trimmed content + its [start,end) offsets. */
export type Seg = { text: string; start: number; end: number };

/** One alignment "bead": source segment indices ↔ target segment indices. */
export type Bead = { src: number[]; tgt: number[] };

export type Alignment = {
  srcSegs: Seg[];
  tgtSegs: Seg[];
  beads: Bead[];
  /** Bead index for each source / target segment (-1 if unmatched/skip). */
  srcBeadOf: number[];
  tgtBeadOf: number[];
};

const EMPTY: Alignment = { srcSegs: [], tgtSegs: [], beads: [], srcBeadOf: [], tgtBeadOf: [] };

/** Paragraph ranges (content separated by blank lines), with offsets. */
function paragraphRanges(text: string): Seg[] {
  const out: Seg[] = [];
  let idx = 0;
  for (const chunk of text.split(/(\n[ \t]*\n)/)) {
    if (!/^\n[ \t]*\n$/.test(chunk)) {
      const lead = chunk.length - chunk.trimStart().length;
      const trimmed = chunk.trim();
      if (trimmed) out.push({ text: trimmed, start: idx + lead, end: idx + lead + trimmed.length });
    }
    idx += chunk.length;
  }
  return out;
}

/** Sentence ranges within a paragraph range, with offsets into the full text. */
function sentenceRanges(fullText: string, lang: string, range: { start: number; end: number }): Seg[] {
  const sub = fullText.slice(range.start, range.end);
  const out: Seg[] = [];
  const Segmenter = (Intl as unknown as { Segmenter?: any }).Segmenter;
  if (Segmenter) {
    try {
      const seg = new Segmenter(lang || "en", { granularity: "sentence" });
      for (const part of seg.segment(sub)) {
        const raw = String(part.segment);
        const trimmed = raw.trim();
        if (!trimmed) continue;
        const lead = raw.length - raw.trimStart().length;
        const start = range.start + part.index + lead;
        out.push({ text: trimmed, start, end: start + trimmed.length });
      }
      if (out.length) return out;
    } catch {
      /* fall through */
    }
  }
  const trimmed = sub.trim();
  if (trimmed) {
    const lead = sub.length - sub.trimStart().length;
    out.push({ text: trimmed, start: range.start + lead, end: range.start + lead + trimmed.length });
  }
  return out;
}

/** Length-based DP alignment (Gale–Church). Beads use LOCAL indices into s/t. */
function galeChurch(s: number[], t: number[], ratio: number): Bead[] {
  const n = s.length;
  const m = t.length;
  const INF = 1e18;
  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(INF));
  const back: (null | [number, number, Bead])[][] = Array.from(
    { length: n + 1 },
    () => new Array(m + 1).fill(null)
  );
  dp[0][0] = 0;
  const lenCost = (sl: number, tl: number) => {
    const exp = sl * ratio;
    return ((tl - exp) * (tl - exp)) / (exp + 1);
  };
  const P_SKIP = 30;
  const P_MERGE = 6;
  for (let i = 0; i <= n; i++) {
    for (let j = 0; j <= m; j++) {
      const cur = dp[i][j];
      if (cur >= INF) continue;
      const relax = (ni: number, nj: number, cost: number, bead: Bead) => {
        if (cur + cost < dp[ni][nj]) {
          dp[ni][nj] = cur + cost;
          back[ni][nj] = [i, j, bead];
        }
      };
      if (i < n && j < m) relax(i + 1, j + 1, lenCost(s[i], t[j]), { src: [i], tgt: [j] });
      if (i < n) relax(i + 1, j, P_SKIP, { src: [i], tgt: [] });
      if (j < m) relax(i, j + 1, P_SKIP, { src: [], tgt: [j] });
      if (i + 1 < n && j < m)
        relax(i + 2, j + 1, lenCost(s[i] + s[i + 1], t[j]) + P_MERGE, { src: [i, i + 1], tgt: [j] });
      if (i < n && j + 1 < m)
        relax(i + 1, j + 2, lenCost(s[i], t[j] + t[j + 1]) + P_MERGE, { src: [i], tgt: [j, j + 1] });
      if (i + 1 < n && j + 1 < m)
        relax(i + 2, j + 2, lenCost(s[i] + s[i + 1], t[j] + t[j + 1]) + P_MERGE, {
          src: [i, i + 1],
          tgt: [j, j + 1],
        });
    }
  }
  const beads: Bead[] = [];
  let i = n;
  let j = m;
  while (!(i === 0 && j === 0)) {
    const b = back[i][j];
    if (!b) break;
    beads.push(b[2]);
    i = b[0];
    j = b[1];
  }
  beads.reverse();
  return beads;
}

function buildBeadMaps(beads: Bead[], nSrc: number, nTgt: number) {
  const srcBeadOf = new Array(nSrc).fill(-1);
  const tgtBeadOf = new Array(nTgt).fill(-1);
  beads.forEach((b, idx) => {
    if (b.src.length && b.tgt.length) {
      b.src.forEach((i) => (srcBeadOf[i] = idx));
      b.tgt.forEach((j) => (tgtBeadOf[j] = idx));
    }
  });
  return { srcBeadOf, tgtBeadOf };
}

const lensOf = (segs: Seg[]) => segs.map((s) => s.text.length);

export function alignText(
  src: string,
  tgt: string,
  mode: LinkMode,
  srcLang: string,
  tgtLang: string
): Alignment {
  if (mode === "off" || !src.trim() || !tgt.trim()) return EMPTY;
  const ratio = tgt.length / (src.length || 1);

  let srcSegs: Seg[];
  let tgtSegs: Seg[];
  let beads: Bead[];

  if (mode === "paragraph") {
    srcSegs = paragraphRanges(src);
    tgtSegs = paragraphRanges(tgt);
    beads = galeChurch(lensOf(srcSegs), lensOf(tgtSegs), ratio);
  } else {
    const sp = paragraphRanges(src);
    const tp = paragraphRanges(tgt);
    srcSegs = [];
    tgtSegs = [];
    beads = [];
    const alignPair = (sr: { start: number; end: number }, tr: { start: number; end: number }) => {
      const ss = sentenceRanges(src, srcLang, sr);
      const ts = sentenceRanges(tgt, tgtLang, tr);
      const sOff = srcSegs.length;
      const tOff = tgtSegs.length;
      const local = galeChurch(lensOf(ss), lensOf(ts), ratio);
      for (const b of local) {
        beads.push({ src: b.src.map((i) => i + sOff), tgt: b.tgt.map((j) => j + tOff) });
      }
      srcSegs.push(...ss);
      tgtSegs.push(...ts);
    };
    if (sp.length === tp.length && sp.length > 0) {
      for (let p = 0; p < sp.length; p++) alignPair(sp[p], tp[p]);
    } else {
      alignPair({ start: 0, end: src.length }, { start: 0, end: tgt.length });
    }
  }

  const { srcBeadOf, tgtBeadOf } = buildBeadMaps(beads, srcSegs.length, tgtSegs.length);
  return { srcSegs, tgtSegs, beads, srcBeadOf, tgtBeadOf };
}
