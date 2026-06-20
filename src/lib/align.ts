// Post-hoc alignment between source text and its translation, fully in the
// frontend (no backend / model / pipeline changes).
//
// Sentences drift with plain index pairing because the model occasionally
// merges or splits a sentence and the error accumulates over the whole text.
// Instead we use a length-based dynamic-programming alignment (Gale–Church):
// it allows 1:1, 1:2, 2:1 and skip beads, so merges/splits are matched rather
// than shifting everything after them. For sentence mode we also bound the
// drift to within a paragraph (paragraphs are stable), aligning sentences only
// inside each paragraph pair.

export type LinkMode = "off" | "sentence" | "paragraph";

/** One alignment "bead": the source segments that map to the target segments. */
export type Bead = { src: number[]; tgt: number[] };

export type Alignment = {
  srcSegs: string[];
  tgtSegs: string[];
  beads: Bead[];
  /** Bead index for each source / target segment (-1 if unmatched/skip). */
  srcBeadOf: number[];
  tgtBeadOf: number[];
};

function splitParagraphs(text: string): string[] {
  return text.split(/\n\s*\n/).map((s) => s.trim()).filter(Boolean);
}

function splitSentences(text: string, lang: string): string[] {
  if (!text.trim()) return [];
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
      /* fall through */
    }
  }
  return text.split(/(?<=[.!?…])\s+/).map((s) => s.trim()).filter(Boolean);
}

/**
 * Length-based DP alignment. Returns beads with LOCAL indices into `s` / `t`.
 * Cost = normalized squared length error + a penalty for non-1:1 beads, so the
 * aligner prefers 1:1 but will merge/split or skip when the lengths demand it.
 */
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
  const P_SKIP = 30; // strongly discourage leaving a segment unmatched
  const P_MERGE = 6; // mild penalty for 1:2 / 2:1 / 2:2

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

const lensOf = (segs: string[]) => segs.map((s) => s.length);
const ratioOf = (src: string[], tgt: string[]) => {
  const a = src.reduce((s, x) => s + x.length, 0);
  const b = tgt.reduce((s, x) => s + x.length, 0);
  return a > 0 ? b / a : 1;
};

function buildBeadMaps(beads: Bead[], nSrc: number, nTgt: number) {
  const srcBeadOf = new Array(nSrc).fill(-1);
  const tgtBeadOf = new Array(nTgt).fill(-1);
  beads.forEach((b, idx) => {
    // Only mark beads that actually link both sides (not pure skips).
    if (b.src.length && b.tgt.length) {
      b.src.forEach((i) => (srcBeadOf[i] = idx));
      b.tgt.forEach((j) => (tgtBeadOf[j] = idx));
    }
  });
  return { srcBeadOf, tgtBeadOf };
}

export function alignText(
  src: string,
  tgt: string,
  mode: LinkMode,
  srcLang: string,
  tgtLang: string
): Alignment {
  if (mode === "off" || !src.trim() || !tgt.trim()) {
    return { srcSegs: [], tgtSegs: [], beads: [], srcBeadOf: [], tgtBeadOf: [] };
  }

  if (mode === "paragraph") {
    const srcSegs = splitParagraphs(src);
    const tgtSegs = splitParagraphs(tgt);
    const beads = galeChurch(lensOf(srcSegs), lensOf(tgtSegs), ratioOf(srcSegs, tgtSegs));
    const { srcBeadOf, tgtBeadOf } = buildBeadMaps(beads, srcSegs.length, tgtSegs.length);
    return { srcSegs, tgtSegs, beads, srcBeadOf, tgtBeadOf };
  }

  // Sentence mode: bound drift to within a paragraph when paragraph counts
  // match; otherwise fall back to a single whole-text sentence alignment.
  const srcParas = splitParagraphs(src);
  const tgtParas = splitParagraphs(tgt);

  const srcSegs: string[] = [];
  const tgtSegs: string[] = [];
  const beads: Bead[] = [];
  const ratio = ratioOf([src], [tgt]);

  const alignPair = (sText: string, tText: string) => {
    const ss = splitSentences(sText, srcLang);
    const ts = splitSentences(tText, tgtLang);
    const sOff = srcSegs.length;
    const tOff = tgtSegs.length;
    const local = galeChurch(lensOf(ss), lensOf(ts), ratio);
    for (const b of local) {
      beads.push({ src: b.src.map((i) => i + sOff), tgt: b.tgt.map((j) => j + tOff) });
    }
    srcSegs.push(...ss);
    tgtSegs.push(...ts);
  };

  if (srcParas.length === tgtParas.length && srcParas.length > 0) {
    for (let p = 0; p < srcParas.length; p++) alignPair(srcParas[p], tgtParas[p]);
  } else {
    alignPair(src, tgt);
  }

  const { srcBeadOf, tgtBeadOf } = buildBeadMaps(beads, srcSegs.length, tgtSegs.length);
  return { srcSegs, tgtSegs, beads, srcBeadOf, tgtBeadOf };
}
